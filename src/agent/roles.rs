//! Agent 角色定义
//!
//! 为不同用途的子 Agent 提供角色枚举和对应的系统提示词模板。

use serde::{Deserialize, Serialize};

/// Agent 角色
///
/// 对标 Claude Code 内置 Agent 体系：
/// - Plan: 计划制定（read-only 探索，输出执行计划）
/// - Verification: 代码验证（对抗性测试，尝试破坏代码）
/// - Guide: 文档问答（回答关于工具/项目的问题）
/// - Advisor: 架构建议（高层设计、重构建议）
/// - Fast: 快速任务（简单、低风险、快速完成）
/// - Teammate: 协作队友（结对编程风格）
/// - Specialist: 领域专家（深度专注）
/// - DreamTask: 探索型规划者（长周期、speculative）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// 默认通用子 Agent
    #[default]
    Default,
    /// 计划制定 Agent（read-only 探索，输出执行计划）
    /// 对标 Claude Code 的 planAgent
    Plan,
    /// 代码验证 Agent（对抗性测试）
    /// 对标 Claude Code 的 verificationAgent
    Verification,
    /// 文档问答 Agent（回答关于工具/项目的问题）
    /// 对标 Claude Code 的 claudeCodeGuideAgent
    Guide,
    /// 架构建议 Agent（高层设计、重构建议）
    /// 对标 Claude Code 的 advisorAgent
    Advisor,
    /// 快速任务 Agent（简单、低风险、快速完成）
    /// 对标 Claude Code 的 fastAgent
    Fast,
    /// 协作型队友（结对编程风格）
    Teammate,
    /// 领域专家（深度专注特定任务）
    Specialist,
    /// 探索型任务规划者（长周期、 speculative）
    DreamTask,
}

impl AgentRole {
    /// 从字符串解析角色（不区分大小写）
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "default" => Some(AgentRole::Default),
            "plan" => Some(AgentRole::Plan),
            "verification" | "verify" => Some(AgentRole::Verification),
            "guide" => Some(AgentRole::Guide),
            "advisor" | "adviser" => Some(AgentRole::Advisor),
            "fast" => Some(AgentRole::Fast),
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
            AgentRole::Plan => {
                "You are a planning specialist. Your job is to analyze tasks and create detailed, actionable plans.\n\
                 Rules:\n\
                 1. EXPLORE first: read relevant files before planning\n\
                 2. Plans must be specific: include exact file paths, function names, and edit locations\n\
                 3. Identify risks and blockers upfront\n\
                 4. Order steps by dependency (what must happen before what)\n\
                 5. You are read-only: do NOT modify any files\n\
                 6. Output your plan in structured markdown"
            }
            AgentRole::Verification => {
                "You are an adversarial verification expert. Your job is to find bugs, not confirm correctness.\n\
                 Rules:\n\
                 1. Try to BREAK the code, not validate it\n\
                 2. Check edge cases, error handling, concurrency, and security\n\
                 3. Run tests and verify they actually test what they claim\n\
                 4. Look for: missing validations, race conditions, resource leaks, injection vulnerabilities\n\
                 5. Be skeptical: assume the code has bugs until proven otherwise\n\
                 6. Output a verdict (PASS/FAIL/PARTIAL) with specific evidence"
            }
            AgentRole::Guide => {
                "You are a knowledgeable guide for this codebase. Answer questions about tools, architecture, and conventions.\n\
                 Rules:\n\
                 1. Reference specific files and line numbers when possible\n\
                 2. Explain the WHY, not just the WHAT\n\
                 3. If unsure, say so — do not hallucinate\n\
                 4. Point users to relevant documentation (README, CLAUDE.md, docs/)\n\
                 5. Keep answers focused and actionable"
            }
            AgentRole::Advisor => {
                "You are a senior architecture advisor. Provide high-level design guidance and strategic recommendations.\n\
                 Rules:\n\
                 1. Consider trade-offs: performance vs maintainability vs complexity\n\
                 2. Reference established patterns and best practices\n\
                 3. Flag technical debt and suggest incremental improvement paths\n\
                 4. Recommend specific libraries, approaches, or refactorings\n\
                 5. Think about the long-term evolution of the codebase"
            }
            AgentRole::Fast => {
                "You are a fast-task executor. Handle simple, low-risk tasks with minimal overhead.\n\
                 Rules:\n\
                 1. Do NOT over-engineer: simplest correct solution wins\n\
                 2. Skip elaborate planning for obvious changes\n\
                 3. Prefer small, focused edits over large refactoring\n\
                 4. If a task seems complex, escalate to a specialist instead\n\
                 5. Move fast but do not break things"
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
            AgentRole::Plan => "plan",
            AgentRole::Verification => "verification",
            AgentRole::Guide => "guide",
            AgentRole::Advisor => "advisor",
            AgentRole::Fast => "fast",
            AgentRole::Teammate => "teammate",
            AgentRole::Specialist => "specialist",
            AgentRole::DreamTask => "dream_task",
        }
    }
}

impl std::str::FromStr for AgentRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown agent role: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_parsing() {
        assert_eq!(AgentRole::parse("teammate"), Some(AgentRole::Teammate));
        assert_eq!(AgentRole::parse("Teammate"), Some(AgentRole::Teammate));
        assert_eq!(AgentRole::parse("dream_task"), Some(AgentRole::DreamTask));
        assert_eq!(AgentRole::parse("dreamtask"), Some(AgentRole::DreamTask));
        assert_eq!(AgentRole::parse("plan"), Some(AgentRole::Plan));
        assert_eq!(
            AgentRole::parse("verification"),
            Some(AgentRole::Verification)
        );
        assert_eq!(AgentRole::parse("verify"), Some(AgentRole::Verification));
        assert_eq!(AgentRole::parse("guide"), Some(AgentRole::Guide));
        assert_eq!(AgentRole::parse("advisor"), Some(AgentRole::Advisor));
        assert_eq!(AgentRole::parse("adviser"), Some(AgentRole::Advisor));
        assert_eq!(AgentRole::parse("fast"), Some(AgentRole::Fast));
        assert_eq!(AgentRole::parse("unknown"), None);
    }

    #[test]
    fn test_role_prefixes_non_empty() {
        for role in [
            AgentRole::Default,
            AgentRole::Plan,
            AgentRole::Verification,
            AgentRole::Guide,
            AgentRole::Advisor,
            AgentRole::Fast,
            AgentRole::Teammate,
            AgentRole::Specialist,
            AgentRole::DreamTask,
        ] {
            assert!(
                !role.system_prompt_prefix().is_empty(),
                "role {:?} has empty prefix",
                role
            );
        }
    }
}

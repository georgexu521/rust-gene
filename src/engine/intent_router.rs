//! Lightweight intent routing before a turn enters the model/tool loop.
//!
//! V1 is deliberately rule-based and advisory. It records the expected workflow,
//! retrieval depth, reasoning depth, and risk so the runtime can be inspected in
//! `/trace` without changing existing behavior prematurely.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    DirectAnswer,
    CodeChange,
    Debugging,
    Research,
    Memory,
    Configuration,
    Delegation,
    Planning,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    Direct,
    CodeChange,
    BugFix,
    Research,
    Planning,
    Delegation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalPolicy {
    None,
    Light,
    Project,
    Memory,
    Web,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningPolicy {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentRoute {
    pub intent: IntentKind,
    pub confidence: f32,
    pub workflow: WorkflowKind,
    pub retrieval: RetrievalPolicy,
    pub reasoning: ReasoningPolicy,
    pub risk: RiskLevel,
    pub recommended_tools: Vec<String>,
    pub reason: String,
}

impl IntentRoute {
    pub fn compact_label(&self) -> String {
        format!("{:?}/{:?}/{:?}", self.intent, self.workflow, self.retrieval)
    }
}

#[derive(Debug, Default, Clone)]
pub struct IntentRouter;

impl IntentRouter {
    pub fn new() -> Self {
        Self
    }

    pub fn route(&self, user_message: &str) -> IntentRoute {
        let text = user_message.trim();
        let lower = text.to_ascii_lowercase();
        let zh = text;

        if text.is_empty() {
            return self.direct("empty prompt", 0.3);
        }

        if contains_any(&lower, &["/memory", "remember", "memory", "recall"])
            || contains_any(zh, &["记忆", "记住", "回忆"])
        {
            return IntentRoute {
                intent: IntentKind::Memory,
                confidence: 0.82,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Memory,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Low,
                recommended_tools: vec!["memory_load".into(), "memory_save".into()],
                reason: "prompt explicitly references memory".into(),
            };
        }

        if contains_any(
            &lower,
            &[
                "config",
                "settings",
                "permission",
                "model",
                "provider",
                "mcp",
            ],
        ) || contains_any(zh, &["配置", "设置", "权限", "模型"])
        {
            return IntentRoute {
                intent: IntentKind::Configuration,
                confidence: 0.78,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["config".into(), "mcp".into()],
                reason: "prompt asks about runtime configuration or permissions".into(),
            };
        }

        if contains_any(
            &lower,
            &["delegate", "subagent", "agent", "parallel", "swarm"],
        ) || contains_any(zh, &["子agent", "子 agent", "并行", "委派"])
        {
            return IntentRoute {
                intent: IntentKind::Delegation,
                confidence: 0.76,
                workflow: WorkflowKind::Delegation,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["agent".into(), "swarm".into(), "project_list".into()],
                reason: "prompt asks for delegation or parallel agent work".into(),
            };
        }

        if contains_any(
            &lower,
            &["plan", "roadmap", "design", "architecture", "refactor"],
        ) || contains_any(zh, &["计划", "路线图", "架构", "重构", "设计"])
        {
            return IntentRoute {
                intent: IntentKind::Planning,
                confidence: 0.74,
                workflow: WorkflowKind::Planning,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["project_list".into(), "grep".into(), "plan".into()],
                reason: "prompt asks for planning or architecture work".into(),
            };
        }

        if contains_any(
            &lower,
            &["fix", "bug", "error", "panic", "fail", "failing", "debug"],
        ) || contains_any(zh, &["报错", "错误", "修复", "失败", "调试", "bug"])
        {
            return IntentRoute {
                intent: IntentKind::Debugging,
                confidence: 0.8,
                workflow: WorkflowKind::BugFix,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["grep".into(), "file_read".into(), "bash".into()],
                reason: "prompt describes a failure or debugging task".into(),
            };
        }

        if contains_any(
            &lower,
            &[
                "implement",
                "add ",
                "change",
                "update",
                "edit",
                "build",
                "optimize",
            ],
        ) || contains_any(zh, &["实现", "新增", "修改", "优化", "完善", "开发"])
        {
            return IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.77,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec![
                    "project_list".into(),
                    "grep".into(),
                    "file_read".into(),
                    "file_edit".into(),
                ],
                reason: "prompt asks for code or product changes".into(),
            };
        }

        if contains_any(&lower, &["search", "web", "latest", "compare", "research"])
            || contains_any(zh, &["搜索", "网上", "最新", "对比", "调研"])
        {
            return IntentRoute {
                intent: IntentKind::Research,
                confidence: 0.72,
                workflow: WorkflowKind::Research,
                retrieval: RetrievalPolicy::Web,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Low,
                recommended_tools: vec!["web_search".into(), "web_fetch".into()],
                reason: "prompt asks for external research or comparison".into(),
            };
        }

        self.direct("no high-risk or multi-step signals detected", 0.66)
    }

    fn direct(&self, reason: impl Into<String>, confidence: f32) -> IntentRoute {
        IntentRoute {
            intent: IntentKind::DirectAnswer,
            confidence,
            workflow: WorkflowKind::Direct,
            retrieval: RetrievalPolicy::Light,
            reasoning: ReasoningPolicy::Low,
            risk: RiskLevel::Low,
            recommended_tools: Vec::new(),
            reason: reason.into(),
        }
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_debugging_tasks() {
        let route = IntentRouter::new().route("cargo test 报错了，帮我修复");
        assert_eq!(route.intent, IntentKind::Debugging);
        assert_eq!(route.workflow, WorkflowKind::BugFix);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn routes_code_change_tasks() {
        let route = IntentRouter::new().route("继续开发 tui 界面，优化状态栏");
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
    }

    #[test]
    fn routes_memory_tasks() {
        let route = IntentRouter::new().route("记住我喜欢 compact 状态栏");
        assert_eq!(route.intent, IntentKind::Memory);
        assert_eq!(route.retrieval, RetrievalPolicy::Memory);
    }

    #[test]
    fn direct_for_simple_question() {
        let route = IntentRouter::new().route("你好");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
    }
}

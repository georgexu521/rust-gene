//! Lightweight intent routing before a turn enters the model/tool loop.
//!
//! V1 is deliberately rule-based and advisory. It records the expected workflow,
//! retrieval depth, reasoning depth, and risk so the runtime can be inspected in
//! `/trace` without changing existing behavior prematurely.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    pub fn route_with_learning(
        &self,
        user_message: &str,
        events: &[crate::session_store::LearningEventRecord],
    ) -> IntentRoute {
        let mut route = self.route(user_message);
        let feedback = LearningFeedback::from_events(events);
        feedback.apply(&mut route);
        route
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

#[derive(Debug, Default)]
struct LearningFeedback {
    recent_failures_for_intent: usize,
    recent_recovery_plans: usize,
    preferred_tools: Vec<String>,
    discouraged_tools: Vec<String>,
}

impl LearningFeedback {
    fn from_events(events: &[crate::session_store::LearningEventRecord]) -> Self {
        let mut feedback = Self::default();
        for event in events.iter().take(20) {
            if event.kind == "recovery_plan" {
                feedback.recent_recovery_plans += 1;
                if event.summary.contains("compact") {
                    feedback.preferred_tools.push("compact".to_string());
                }
            }
            if event.kind == "turn_outcome" {
                let status = event
                    .payload
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if status != "Completed" {
                    feedback.recent_failures_for_intent += 1;
                }
                if let Some(intent) = event.payload.get("intent").and_then(|v| v.as_str()) {
                    match intent {
                        "CodeChange" | "Debugging" => {
                            feedback.preferred_tools.push("grep".to_string());
                            feedback.preferred_tools.push("file_read".to_string());
                        }
                        "Research" => feedback.preferred_tools.push("web_search".to_string()),
                        _ => {}
                    }
                }
            }
            if event.kind == "tool_outcome" {
                let tool = event
                    .payload
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let success = event
                    .payload
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !tool.is_empty() {
                    if success {
                        feedback.preferred_tools.push(tool.to_string());
                    } else {
                        feedback.discouraged_tools.push(tool.to_string());
                    }
                }
            }
        }
        feedback.preferred_tools.sort();
        feedback.preferred_tools.dedup();
        feedback.discouraged_tools = repeated_tools(&feedback.discouraged_tools, 2);
        feedback
            .preferred_tools
            .retain(|tool| !feedback.discouraged_tools.contains(tool));
        feedback
    }

    fn apply(&self, route: &mut IntentRoute) {
        if self.recent_recovery_plans > 0 {
            route.confidence = (route.confidence - 0.05).max(0.1);
            route.reason.push_str(&format!(
                "; learning feedback: {} recent recovery plan(s)",
                self.recent_recovery_plans
            ));
            if route.retrieval == RetrievalPolicy::Light {
                route.retrieval = RetrievalPolicy::Project;
            }
        }
        if self.recent_failures_for_intent >= 2 {
            route.confidence = (route.confidence - 0.1).max(0.1);
            route
                .reason
                .push_str("; learning feedback: recent failed turns, use more context");
            if matches!(
                route.reasoning,
                ReasoningPolicy::Low | ReasoningPolicy::Medium
            ) {
                route.reasoning = ReasoningPolicy::High;
            }
            if matches!(route.risk, RiskLevel::Low) {
                route.risk = RiskLevel::Medium;
            }
        }
        for tool in &self.preferred_tools {
            if !route.recommended_tools.contains(tool) {
                route.recommended_tools.push(tool.clone());
            }
        }
        if !self.discouraged_tools.is_empty() {
            let before = route.recommended_tools.len();
            route
                .recommended_tools
                .retain(|tool| !self.discouraged_tools.contains(tool));
            let removed = before.saturating_sub(route.recommended_tools.len());
            if removed > 0 {
                route.confidence = (route.confidence - 0.05).max(0.1);
                route.reason.push_str(&format!(
                    "; learning feedback: avoided recently failing tool(s): {}",
                    self.discouraged_tools.join(", ")
                ));
            }
        }
    }
}

fn repeated_tools(tools: &[String], min_count: usize) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for tool in tools {
        *counts.entry(tool.clone()).or_default() += 1;
    }
    let mut repeated = counts
        .into_iter()
        .filter_map(|(tool, count)| (count >= min_count).then_some(tool))
        .collect::<Vec<_>>();
    repeated.sort();
    repeated
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

    #[test]
    fn learning_feedback_raises_caution_after_failures() {
        let events = vec![
            crate::session_store::LearningEventRecord {
                id: 1,
                session_id: "s1".to_string(),
                kind: "turn_outcome".to_string(),
                source: "test".to_string(),
                summary: "failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"status": "Failed", "intent": "CodeChange"}),
                created_at: "now".to_string(),
            },
            crate::session_store::LearningEventRecord {
                id: 2,
                session_id: "s1".to_string(),
                kind: "turn_outcome".to_string(),
                source: "test".to_string(),
                summary: "failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"status": "Failed", "intent": "CodeChange"}),
                created_at: "now".to_string(),
            },
        ];
        let route = IntentRouter::new().route_with_learning("你好", &events);
        assert_eq!(route.reasoning, ReasoningPolicy::High);
        assert_eq!(route.risk, RiskLevel::Medium);
        assert!(route.reason.contains("learning feedback"));
    }

    #[test]
    fn learning_feedback_discourages_repeated_tool_failures() {
        let events = vec![
            crate::session_store::LearningEventRecord {
                id: 1,
                session_id: "s1".to_string(),
                kind: "tool_outcome".to_string(),
                source: "test".to_string(),
                summary: "grep failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"tool": "grep", "success": false}),
                created_at: "now".to_string(),
            },
            crate::session_store::LearningEventRecord {
                id: 2,
                session_id: "s1".to_string(),
                kind: "tool_outcome".to_string(),
                source: "test".to_string(),
                summary: "grep failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"tool": "grep", "success": false}),
                created_at: "now".to_string(),
            },
        ];
        let route = IntentRouter::new().route_with_learning("帮我修复 cargo test 报错", &events);
        assert!(!route.recommended_tools.contains(&"grep".to_string()));
        assert!(route.reason.contains("avoided recently failing"));
    }
}

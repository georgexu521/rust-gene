//! Lightweight intent routing before a turn enters the model/tool loop.
//!
//! V1 is deliberately rule-based and advisory. It records the expected workflow,
//! retrieval depth, reasoning depth, and risk so the runtime can be inspected in
//! `/trace` without changing existing behavior prematurely.
//! A route must not answer locally, grant mutation authority, or override the
//! model's semantic plan; it only shapes optional context, tools, and tracing.

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

impl RetrievalPolicy {
    pub fn allows_memory_context(self) -> bool {
        matches!(self, Self::Memory | Self::Project | Self::Full)
    }

    pub fn allows_project_context(self) -> bool {
        matches!(self, Self::Project | Self::Full)
    }
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
    #[serde(default)]
    pub dependency_install_intent: bool,
    #[serde(default)]
    pub mcp_auth_intent: bool,
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

        let has_live_coding_code_change_signal = is_live_coding_code_change_request(&lower);
        let has_live_coding_audit_signal = is_live_coding_audit_request(&lower);
        let has_memory_signal = contains_any(&lower, &["/memory", "remember", "memory", "recall"])
            || contains_any(zh, &["记忆", "记住", "回忆"]);
        let has_code_creation_signal = is_natural_code_creation_request(&lower, zh);
        let has_code_change_signal = has_live_coding_code_change_signal
            || has_live_coding_audit_signal
            || is_code_change_request(&lower, zh);
        let has_debug_signal = is_debug_request(&lower, zh);
        let has_file_mutation_signal = is_file_mutation_request(&lower, zh);
        let has_local_inspection_signal = is_local_inspection_request(&lower, zh);
        let has_terminal_operation_signal = is_terminal_operation_request(&lower, zh);
        let has_error_explanation_signal = is_error_explanation_request(&lower, zh);
        let has_dependency_install_intent = is_dependency_install_request(&lower, zh);
        let has_mcp_auth_intent = is_mcp_auth_request(&lower, zh);
        let has_configuration_signal = contains_any(
            &lower,
            &[
                "config",
                "settings",
                "permission",
                "model",
                "provider",
                "mcp",
            ],
        ) || contains_any(zh, &["配置", "设置", "权限", "模型"]);
        let has_delegation_signal =
            contains_any(&lower, &["delegate", "subagent", "parallel", "swarm"])
                || contains_any(zh, &["子agent", "子 agent", "并行", "委派"]);
        let has_research_signal =
            contains_any(&lower, &["search", "web", "latest", "compare", "research"])
                || contains_any(zh, &["搜索", "网上", "最新", "对比", "调研"]);
        let has_planning_signal =
            contains_any(
                &lower,
                &["plan", "roadmap", "design", "architecture", "refactor"],
            ) || contains_any(zh, &["计划", "路线图", "架构", "重构", "设计"]);

        // Memory-related coding tasks, such as fixing memory_save or memory
        // scoring, must not be routed as direct memory lookup/save turns. Treat
        // the domain word "memory" as the subject of the code-change request
        // when there are explicit bug-fix or edit signals.
        if has_memory_signal && !has_code_change_signal {
            return IntentRoute {
                intent: IntentKind::Memory,
                confidence: 0.82,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Memory,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Low,
                recommended_tools: vec!["memory_load".into(), "memory_save".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt explicitly references memory without code-change intent".into(),
            };
        }

        if has_live_coding_code_change_signal {
            return IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.88,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: live_coding_risk(&lower),
                recommended_tools: code_change_tool_recommendations_for(
                    has_dependency_install_intent,
                ),
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "live coding eval explicitly requires a code diff".into(),
            };
        }

        if has_live_coding_audit_signal {
            return IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.84,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: live_coding_risk(&lower),
                recommended_tools: code_change_tool_recommendations_for(
                    has_dependency_install_intent,
                ),
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "live coding audit/regression eval requires project verification; code diff is optional".into(),
            };
        }

        if has_error_explanation_signal {
            return IntentRoute {
                intent: IntentKind::DirectAnswer,
                confidence: 0.78,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Low,
                recommended_tools: Vec::new(),
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks to explain an error without a code or environment action"
                    .into(),
            };
        }

        if has_configuration_signal {
            return IntentRoute {
                intent: IntentKind::Configuration,
                confidence: 0.78,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Medium,
                recommended_tools: configuration_tool_recommendations(has_mcp_auth_intent),
                dependency_install_intent: false,
                mcp_auth_intent: has_mcp_auth_intent,
                reason: "prompt asks about runtime configuration or permissions".into(),
            };
        }

        if has_delegation_signal {
            return IntentRoute {
                intent: IntentKind::Delegation,
                confidence: 0.76,
                workflow: WorkflowKind::Delegation,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["agent".into(), "swarm".into(), "project_list".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks for delegation or parallel agent work".into(),
            };
        }

        if has_research_signal {
            return IntentRoute {
                intent: IntentKind::Research,
                confidence: 0.72,
                workflow: WorkflowKind::Research,
                retrieval: RetrievalPolicy::Web,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Low,
                recommended_tools: vec!["web_search".into(), "web_fetch".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks for external research or comparison".into(),
            };
        }

        if has_planning_signal {
            return IntentRoute {
                intent: IntentKind::Planning,
                confidence: 0.74,
                workflow: WorkflowKind::Planning,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["project_list".into(), "grep".into(), "plan".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks for planning or architecture work".into(),
            };
        }

        if has_code_creation_signal {
            return IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.8,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: code_change_tool_recommendations_for(
                    has_dependency_install_intent,
                ),
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "prompt asks to create a code artifact".into(),
            };
        }

        if has_terminal_operation_signal {
            let mut recommended_tools = if is_background_shell_followup(&lower, zh) {
                vec![
                    "bash".into(),
                    "bash_output".into(),
                    "bash_cancel".into(),
                    "bash_tasks".into(),
                ]
            } else {
                vec!["bash".into()]
            };
            maybe_recommend_dependency_install_tool(
                &mut recommended_tools,
                has_dependency_install_intent,
            );
            return IntentRoute {
                intent: IntentKind::DirectAnswer,
                confidence: 0.74,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Medium,
                recommended_tools,
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "prompt asks to inspect or change local runtime state through the terminal"
                    .into(),
            };
        }

        if has_local_inspection_signal {
            return IntentRoute {
                intent: IntentKind::DirectAnswer,
                confidence: 0.72,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Low,
                recommended_tools: vec!["glob".into(), "file_read".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks to inspect local filesystem or workspace state".into(),
            };
        }

        if has_debug_signal {
            return IntentRoute {
                intent: IntentKind::Debugging,
                confidence: 0.8,
                workflow: WorkflowKind::BugFix,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: debug_tool_recommendations(has_dependency_install_intent),
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "prompt describes a failure or debugging task".into(),
            };
        }

        if has_file_mutation_signal {
            return IntentRoute {
                intent: IntentKind::DirectAnswer,
                confidence: 0.74,
                workflow: WorkflowKind::Direct,
                retrieval: RetrievalPolicy::Light,
                reasoning: ReasoningPolicy::Medium,
                risk: RiskLevel::Medium,
                recommended_tools: vec!["file_read".into(), "bash".into()],
                dependency_install_intent: false,
                mcp_auth_intent: false,
                reason: "prompt asks for a scoped file mutation without code workflow intent"
                    .into(),
            };
        }

        if has_code_change_signal {
            return IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.77,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::Medium,
                recommended_tools: code_change_tool_recommendations_for(
                    has_dependency_install_intent,
                ),
                dependency_install_intent: has_dependency_install_intent,
                mcp_auth_intent: false,
                reason: "prompt asks for code or product changes".into(),
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
            dependency_install_intent: false,
            mcp_auth_intent: false,
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
            if tool == "install_dependencies" && !route.dependency_install_intent {
                continue;
            }
            if tool == "mcp_auth" && !route.mcp_auth_intent {
                continue;
            }
            if !route.recommended_tools.contains(tool) {
                route.recommended_tools.push(tool.clone());
            }
        }
        if !self.discouraged_tools.is_empty() {
            route.confidence = (route.confidence - 0.05).max(0.1);
            route.reason.push_str(&format!(
                "; learning feedback: recent failure(s) for tool(s): {}",
                self.discouraged_tools.join(", ")
            ));
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

fn is_debug_request(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "live coding regression task",
            "type: `bug_fix`",
            "type: bug_fix",
            "bug_fix",
            "fix",
            "bug",
            "error",
            "panic",
            "fail",
            "failing",
            "debug",
        ],
    ) || contains_any(zh, &["报错", "错误", "修复", "失败", "调试", "bug"])
        || (zh.contains("问题") && contains_any(zh, &["运行", "启动", "执行", "测试"]))
}

fn is_live_coding_code_change_request(lower: &str) -> bool {
    (lower.contains("live coding regression task") && !is_no_diff_eval_intent(lower))
        || (lower.contains("eval intent") && lower.contains("seeded_code_change"))
        || lower.contains("this is a real code-change evaluation")
}

fn is_live_coding_audit_request(lower: &str) -> bool {
    lower.contains("live coding regression task") && is_no_diff_eval_intent(lower)
}

fn is_no_diff_eval_intent(lower: &str) -> bool {
    lower.contains("eval intent: `audit_or_regression_check`")
        || lower.contains("eval intent: audit_or_regression_check")
        || lower.contains("eval intent: `stale_or_already_satisfied`")
        || lower.contains("eval intent: stale_or_already_satisfied")
}

fn live_coding_risk(lower: &str) -> RiskLevel {
    if lower.contains("risk: `high`")
        || lower.contains("risk: high")
        || lower.contains("- risk: `high`")
    {
        RiskLevel::High
    } else {
        RiskLevel::Medium
    }
}

fn code_change_tool_recommendations() -> Vec<String> {
    vec![
        "project_list".into(),
        "grep".into(),
        "file_read".into(),
        "file_write".into(),
        "file_edit".into(),
        "bash".into(),
    ]
}

fn code_change_tool_recommendations_for(dependency_install_intent: bool) -> Vec<String> {
    let mut tools = code_change_tool_recommendations();
    maybe_recommend_dependency_install_tool(&mut tools, dependency_install_intent);
    tools
}

fn debug_tool_recommendations(dependency_install_intent: bool) -> Vec<String> {
    let mut tools = vec!["grep".into(), "file_read".into(), "bash".into()];
    maybe_recommend_dependency_install_tool(&mut tools, dependency_install_intent);
    tools
}

fn configuration_tool_recommendations(mcp_auth_intent: bool) -> Vec<String> {
    let mut tools = vec!["config".into(), "mcp".into()];
    if mcp_auth_intent {
        tools.push("mcp_auth".into());
    }
    tools
}

fn maybe_recommend_dependency_install_tool(
    tools: &mut Vec<String>,
    dependency_install_intent: bool,
) {
    if dependency_install_intent && !tools.iter().any(|tool| tool == "install_dependencies") {
        tools.push("install_dependencies".into());
    }
}

fn is_dependency_install_request(lower: &str, zh: &str) -> bool {
    if contains_any(
        lower,
        &[
            "do not install",
            "don't install",
            "dont install",
            "without installing",
            "only report",
            "just report",
        ],
    ) || contains_any(zh, &["不要安装", "别安装", "不用安装", "只报告", "仅报告"])
    {
        return false;
    }

    if contains_any(
        lower,
        &[
            "pip install",
            "pip3 install",
            "uv pip install",
            "npm install",
            "npm i ",
            "pnpm install",
            "pnpm i ",
            "yarn install",
            "yarn add",
            "cargo add",
            "go get",
            "poetry add",
            "poetry install",
        ],
    ) {
        return true;
    }

    let explicit_dependency_phrase = contains_any(
        lower,
        &[
            "install dependencies",
            "install deps",
            "install packages",
            "install package",
            "add dependency",
            "add dependencies",
            "add package",
            "add packages",
        ],
    ) || contains_any(
        zh,
        &[
            "安装依赖",
            "装依赖",
            "依赖安装",
            "安装包",
            "装包",
            "安装模块",
            "装模块",
            "安装库",
            "装库",
            "补依赖",
            "加依赖",
            "添加依赖",
        ],
    );
    if explicit_dependency_phrase {
        return true;
    }

    let package_context = contains_any(
        lower,
        &[
            "dependency",
            "dependencies",
            "package",
            "module",
            "library",
            "requirements.txt",
            "package.json",
            "cargo.toml",
            "go.mod",
            "pip",
            "pip3",
            "npm",
            "pnpm",
            "yarn",
            "poetry",
            "python",
            "python3",
            "node",
            "rust",
            "go",
        ],
    ) || contains_any(zh, &["依赖", "包", "模块", "库"]);
    let explicit_install_action =
        contains_any(lower, &["install ", "add package", "add dependency"])
            || contains_any(zh, &["帮我安装", "帮我装", "安装一下", "装一下"]);

    package_context && explicit_install_action
}

fn is_mcp_auth_request(lower: &str, zh: &str) -> bool {
    let has_mcp_subject = lower.contains("mcp") || zh.contains("MCP") || zh.contains("mcp");
    if !has_mcp_subject {
        return false;
    }
    contains_any(
        lower,
        &[
            "auth",
            "authenticate",
            "authentication",
            "authorize",
            "authorization",
            "oauth",
            "login",
            "log in",
            "token",
            "credential",
        ],
    ) || contains_any(
        zh,
        &["认证", "授权", "登录", "登陆", "令牌", "凭据", "token"],
    )
}

fn is_error_explanation_request(lower: &str, zh: &str) -> bool {
    let has_error_subject = contains_any(
        lower,
        &[
            "error",
            "exception",
            "stack trace",
            "traceback",
            "bad_request",
            "status 400",
            "failed to",
        ],
    ) || contains_any(zh, &["报错", "错误", "异常"]);
    let asks_for_explanation = contains_any(
        lower,
        &[
            "what does",
            "what is",
            "what means",
            "what does this mean",
            "explain",
            "why",
            "原因",
        ],
    ) || contains_any(
        zh,
        &[
            "是什么意思",
            "什么意思",
            "解释",
            "原因是什么",
            "为什么",
            "怎么回事",
        ],
    );
    let asks_for_repair_or_action = contains_any(
        lower,
        &[
            "fix", "repair", "solve", "resolve", "debug", "change", "edit",
        ],
    ) || contains_any(zh, &["修复", "解决", "改", "修改", "调试"]);

    has_error_subject && asks_for_explanation && !asks_for_repair_or_action
}

fn is_code_change_request(lower: &str, zh: &str) -> bool {
    is_debug_request(lower, zh)
        || is_natural_code_creation_request(lower, zh)
        || contains_any(
            lower,
            &[
                "implement",
                "add ",
                "change",
                "update",
                "edit",
                "build",
                "optimize",
                "refactor",
            ],
        )
        || contains_any(
            zh,
            &["实现", "新增", "修改", "优化", "完善", "开发", "重构"],
        )
}

fn is_natural_code_creation_request(lower: &str, zh: &str) -> bool {
    let has_creation_verb = contains_any(
        lower,
        &[
            "create", "make", "generate", "write a", "write an", "build a",
        ],
    ) || contains_any(
        zh,
        &[
            "做",
            "写",
            "弄一个",
            "做一个",
            "写一个",
            "创建一个",
            "创建一",
            "生成一个",
            "生成一",
        ],
    );
    has_creation_verb && has_code_artifact_signal(lower, zh)
}

fn has_code_artifact_signal(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "python",
            ".py",
            "html",
            "javascript",
            "typescript",
            "rust",
            "shell",
            "bash",
            "script",
            "game",
            "app",
            "program",
            "code",
            "snake",
        ],
    ) || contains_any(
        zh,
        &[
            "脚本",
            "游戏",
            "页面",
            "网页",
            "程序",
            "代码",
            "应用",
            "贪吃蛇",
        ],
    )
}

fn is_file_mutation_request(lower: &str, zh: &str) -> bool {
    let has_mutation = contains_any(lower, &["delete", "remove", "rename", "move", "trash"])
        || contains_any(zh, &["删除", "删掉", "删了", "移除", "重命名", "移动"]);
    let has_file_scope = contains_any(
        lower,
        &["file", "folder", "directory", ".txt", ".md", ".json", ".py"],
    ) || contains_any(zh, &["文件", "文件夹", "目录"]);
    has_mutation && has_file_scope
}

fn is_terminal_operation_request(lower: &str, zh: &str) -> bool {
    let terminal_subject = contains_any(
        lower,
        &[
            "terminal",
            "shell",
            "bash",
            "command",
            "pip",
            "pip3",
            "python",
            "python3",
            "npm",
            "pnpm",
            "yarn",
            "node",
            "cargo",
            "brew",
            "docker",
            "pytest",
            "venv",
            "virtualenv",
            "background",
            "handle",
            "process",
            "server",
            "watch",
        ],
    ) || contains_any(
        zh,
        &[
            "终端",
            "命令",
            "默认的python",
            "默认 python",
            "默认python",
            "依赖",
            "模块",
            "环境",
            "后台",
            "句柄",
            "进程",
            "服务器",
        ],
    );
    let terminal_action = contains_any(
        lower,
        &[
            "install",
            "uninstall",
            "run",
            "execute",
            "start",
            "launch",
            "check",
            "list",
            "show",
            "status",
            "version",
            "which ",
            "where ",
            "is installed",
            "package",
            "dependency",
            "read output",
            "cancel",
            "stop",
        ],
    ) || contains_any(
        zh,
        &[
            "安装",
            "卸载",
            "运行",
            "执行",
            "启动",
            "检查",
            "查看",
            "看看",
            "有哪些",
            "状态",
            "有没有安装",
            "怎么运行",
            "跑一下",
            "装一下",
            "读取输出",
            "取消",
            "停止",
        ],
    );

    terminal_subject && terminal_action
}

fn is_background_shell_followup(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "background",
            "handle",
            "process",
            "bash_output",
            "bash_cancel",
            "bash_tasks",
        ],
    ) || contains_any(zh, &["后台", "句柄", "进程"])
}

fn is_local_inspection_request(lower: &str, zh: &str) -> bool {
    let asks_to_inspect = contains_any(
        lower,
        &[
            "check", "look", "list", "show", "find", "exists", "exist", "is there", "whether",
            "inside", "contents",
        ],
    ) || contains_any(
        zh,
        &[
            "看看",
            "查看",
            "检查",
            "列出",
            "找找",
            "找一下",
            "有没有",
            "是否存在",
            "在不在",
            "有吗",
            "里面",
            "有什么",
            "内容",
        ],
    );
    let local_scope = contains_any(
        lower,
        &[
            "desktop",
            "~/",
            "/users/",
            "workspace",
            "repo",
            "folder",
            "directory",
            "file",
            ".txt",
            ".md",
            ".json",
            ".py",
        ],
    ) || contains_any(
        zh,
        &["桌面", "工作区", "项目", "仓库", "文件夹", "目录", "文件"],
    );
    let anaphora_scope = contains_any(
        lower,
        &["inside", "there", "that folder", "this folder", "it"],
    ) || contains_any(zh, &["里面", "其中", "这个", "那里", "刚才"]);
    asks_to_inspect && (local_scope || anaphora_scope)
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
    fn routes_running_issue_as_debugging_task() {
        let route = IntentRouter::new().route("我在运行中发现了一个问题，你帮我看看是怎么回事吧");
        assert_eq!(route.intent, IntentKind::Debugging);
        assert_eq!(route.workflow, WorkflowKind::BugFix);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
    }

    #[test]
    fn routes_code_change_tasks() {
        let route = IntentRouter::new().route("继续开发 tui 界面，优化状态栏");
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
    }

    #[test]
    fn routes_natural_chinese_python_game_creation_as_code_change() {
        let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
    }

    #[test]
    fn routes_chinese_html_creation_as_code_change() {
        let route = IntentRouter::new().route("创建一个简单 html 页面");
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
    }

    #[test]
    fn routes_generic_mcp_configuration_without_auth_tool() {
        let route = IntentRouter::new().route("帮我看看 mcp 配置");
        assert_eq!(route.intent, IntentKind::Configuration);
        assert!(route.recommended_tools.contains(&"mcp".to_string()));
        assert!(!route.recommended_tools.contains(&"mcp_auth".to_string()));
        assert!(!route.mcp_auth_intent);
    }

    #[test]
    fn routes_explicit_mcp_auth_with_auth_tool() {
        let route = IntentRouter::new().route("帮我给 mcp server 做 OAuth 授权登录");
        assert_eq!(route.intent, IntentKind::Configuration);
        assert!(route.recommended_tools.contains(&"mcp".to_string()));
        assert!(route.recommended_tools.contains(&"mcp_auth".to_string()));
        assert!(route.mcp_auth_intent);
    }

    #[test]
    fn routes_python_package_install_as_terminal_operation() {
        let route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.risk, RiskLevel::Medium);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
        assert!(route.dependency_install_intent);
    }

    #[test]
    fn routes_run_script_question_as_terminal_operation() {
        let route = IntentRouter::new().route("我该怎么运行这个 python 程序？帮我跑一下");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.risk, RiskLevel::Medium);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn routes_background_shell_handle_question_as_terminal_operation() {
        let route = IntentRouter::new().route("读取这个后台 shell 句柄的输出，然后停止它");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(route.recommended_tools.contains(&"bash_output".to_string()));
        assert!(route.recommended_tools.contains(&"bash_cancel".to_string()));
        assert!(route.recommended_tools.contains(&"bash_tasks".to_string()));
    }

    #[test]
    fn routes_error_explanation_without_action_as_direct_answer() {
        let route = IntentRouter::new().route(
            "这个报错是什么意思？ Error: Failed to get response from MiniMax API: bad_request_error",
        );
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.retrieval, RetrievalPolicy::Light);
        assert!(route.recommended_tools.is_empty());
    }

    #[test]
    fn routes_error_fix_request_as_debugging_task() {
        let route = IntentRouter::new().route("这个报错是什么意思？帮我修复一下");
        assert_eq!(route.intent, IntentKind::Debugging);
        assert_eq!(route.workflow, WorkflowKind::BugFix);
    }

    #[test]
    fn routes_single_file_delete_as_scoped_direct_mutation() {
        let route = IntentRouter::new().route("帮我把这个文件删了吧");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.risk, RiskLevel::Medium);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn routes_desktop_folder_existence_as_local_inspection() {
        let route = IntentRouter::new()
            .route("请帮我看看桌面有没有 gex 文件夹。不要编造大小、创建时间或内容数量。");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.retrieval, RetrievalPolicy::Light);
        assert!(route.recommended_tools.contains(&"glob".to_string()));
        assert!(route.recommended_tools.contains(&"file_read".to_string()));
        assert!(!route.recommended_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn routes_environment_check_with_error_word_as_terminal_operation() {
        let route = IntentRouter::new().route(
            "请帮我检查当前电脑默认 python3 能不能 import pygame。如果没安装，只报告实际错误信息。",
        );
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(!route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
        assert!(!route.dependency_install_intent);
    }

    #[test]
    fn routes_explicit_dependency_install_with_structured_tool() {
        let route = IntentRouter::new().route("帮我安装项目依赖，package.json 已经在项目里");
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
        assert!(route.dependency_install_intent);
    }

    #[test]
    fn routes_code_creation_with_run_and_problem_words_as_code_change() {
        let route = IntentRouter::new().route(
            "请创建一个简单 Python 脚本。脚本运行后打印 hello，写完后验证 Python 语法没问题。",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
    }

    #[test]
    fn live_coding_eval_summary_task_routes_as_code_change() {
        let route = IntentRouter::new().route(
            "# Live coding regression task: live eval reports should summarize pass rates\n\
             - Eval intent: `seeded_code_change`\n\
             当前 seeded worktree 已保留 `scripts/run_live_eval.sh --mode summary` 入口，请修改 summary_task()。",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
        assert!(route.recommended_tools.contains(&"file_edit".to_string()));
    }

    #[test]
    fn live_coding_eval_search_feature_routes_as_code_change_not_research() {
        let route = IntentRouter::new().route(
            "# Live coding regression task: build a small book notes frontend with search, tags, and persistence\n\
             - Eval intent: `seeded_code_change`\n\
             请完成 `fixtures/live_frontend/book_notes` 里的本地书摘记录网站。",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
    }

    #[test]
    fn live_coding_eval_memory_quality_task_routes_as_code_change_not_config() {
        let route = IntentRouter::new().route(
            "# Live coding regression task: memory_save should respect quality gates\n\
             - Eval intent: `seeded_code_change`\n\
             - Risk: `high`\n\
             修复 memory_save 绕过记忆质量门控的问题。",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.risk, RiskLevel::High);
    }

    #[test]
    fn audit_eval_does_not_claim_diff_is_required() {
        let route = IntentRouter::new().route(
            "# Live coding regression task: memory recall should demote only relevant conflicts\n\
             - Type: `bug_fix`\n\
             - Eval intent: `audit_or_regression_check`\n\
             If the requested behavior is already present, prove it with direct evidence.",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
        assert_ne!(
            route.reason,
            "live coding eval explicitly requires a code diff"
        );
        assert!(route.reason.contains("code diff is optional"));
    }

    #[test]
    fn audit_eval_memory_safety_task_routes_as_code_change_not_config() {
        let route = IntentRouter::new().route(
            "# Live coding regression task: explicit memory saves must not persist sensitive data\n\
             - Type: `bug_fix`\n\
             - Eval intent: `audit_or_regression_check`\n\
             - Risk: `high`\n\
             要求：即使用户显式 /save，也不能保存 API key、token、password、private key 等敏感内容。",
        );
        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
        assert_eq!(route.risk, RiskLevel::High);
        assert!(route.recommended_tools.contains(&"file_edit".to_string()));
    }

    #[test]
    fn routes_followup_inside_question_as_local_inspection() {
        let route = IntentRouter::new().route("里面有什么东西");
        assert_eq!(route.intent, IntentKind::DirectAnswer);
        assert_eq!(route.workflow, WorkflowKind::Direct);
        assert_eq!(route.retrieval, RetrievalPolicy::Light);
        assert!(route.recommended_tools.contains(&"file_read".to_string()));
        assert!(!route.recommended_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn routes_agent_design_comparison_as_research_not_delegation() {
        let route = IntentRouter::new().route("帮我对比 claude 和 opencode 的 agent 指令设计");
        assert_eq!(route.intent, IntentKind::Research);
        assert_eq!(route.workflow, WorkflowKind::Research);
        assert_eq!(route.retrieval, RetrievalPolicy::Web);
    }

    #[test]
    fn routes_planning_about_how_to_build_without_forcing_code_change() {
        let route = IntentRouter::new().route("计划一下怎么做贪吃蛇");
        assert_eq!(route.intent, IntentKind::Planning);
        assert_eq!(route.workflow, WorkflowKind::Planning);
    }

    #[test]
    fn planning_signal_wins_over_local_inspection_words() {
        let route = IntentRouter::new().route("帮我看看这个项目，然后写一个修改计划");
        assert_eq!(route.intent, IntentKind::Planning);
        assert_eq!(route.workflow, WorkflowKind::Planning);
    }

    #[test]
    fn routes_memory_tasks() {
        let route = IntentRouter::new().route("记住我喜欢 compact 状态栏");
        assert_eq!(route.intent, IntentKind::Memory);
        assert_eq!(route.retrieval, RetrievalPolicy::Memory);
    }

    #[test]
    fn routes_memory_domain_bugfix_as_code_workflow() {
        let route = IntentRouter::new().route("修复 memory_save 绕过记忆质量门控的问题，新增测试");
        assert_eq!(route.intent, IntentKind::Debugging);
        assert_eq!(route.workflow, WorkflowKind::BugFix);
        assert_eq!(route.retrieval, RetrievalPolicy::Project);
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
    fn learning_feedback_notes_repeated_tool_failures_without_hiding_tools() {
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
        assert!(route.recommended_tools.contains(&"grep".to_string()));
        assert!(route.reason.contains("recent failure"));
    }

    #[test]
    fn learning_feedback_does_not_add_dependency_install_without_intent() {
        let events = vec![crate::session_store::LearningEventRecord {
            id: 1,
            session_id: "s1".to_string(),
            kind: "tool_outcome".to_string(),
            source: "test".to_string(),
            summary: "install succeeded".to_string(),
            confidence: 1.0,
            payload: serde_json::json!({"tool": "install_dependencies", "success": true}),
            created_at: "now".to_string(),
        }];

        let route = IntentRouter::new().route_with_learning("帮我做一个贪吃蛇游戏", &events);

        assert!(!route.dependency_install_intent);
        assert!(!route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
    }

    #[test]
    fn learning_feedback_does_not_add_mcp_auth_without_auth_intent() {
        let events = vec![crate::session_store::LearningEventRecord {
            id: 1,
            session_id: "s1".to_string(),
            kind: "tool_outcome".to_string(),
            source: "test".to_string(),
            summary: "mcp auth succeeded".to_string(),
            confidence: 1.0,
            payload: serde_json::json!({"tool": "mcp_auth", "success": true}),
            created_at: "now".to_string(),
        }];

        let route = IntentRouter::new().route_with_learning("帮我看看 mcp 配置", &events);

        assert!(!route.mcp_auth_intent);
        assert!(!route.recommended_tools.contains(&"mcp_auth".to_string()));
    }

    #[test]
    fn learning_feedback_keeps_bash_for_terminal_operation_after_failures() {
        let events = vec![
            crate::session_store::LearningEventRecord {
                id: 1,
                session_id: "s1".to_string(),
                kind: "tool_outcome".to_string(),
                source: "test".to_string(),
                summary: "bash failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"tool": "bash", "success": false}),
                created_at: "now".to_string(),
            },
            crate::session_store::LearningEventRecord {
                id: 2,
                session_id: "s1".to_string(),
                kind: "tool_outcome".to_string(),
                source: "test".to_string(),
                summary: "bash failed".to_string(),
                confidence: 1.0,
                payload: serde_json::json!({"tool": "bash", "success": false}),
                created_at: "now".to_string(),
            },
        ];

        let route = IntentRouter::new().route_with_learning(
            "帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧",
            &events,
        );

        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(route.reason.contains("recent failure"));
    }
}

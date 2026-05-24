//! 上下文压缩器
//!
//! 参考 hermes-agent 的设计：
//! - Token 预算管理（根据模型上下文窗口动态计算）
//! - 两阶段压缩：先裁剪工具输出，再 LLM 摘要
//! - 8 段结构化摘要模板（Goal/Constraints/Progress/Decisions/Files/Next Steps/Critical Context/Tools & Patterns）
//! - 迭代式摘要更新（累积知识而非丢失）
//! - Token-budget 尾部保护（soft_ceiling = budget * 1.5）
//! - 工具调用对完整性校验（孤立项清理 + stub 插入）

pub use crate::engine::context_collapse::{
    extract_compact_boundaries, CompactMetadata, CompactionAttemptRecord, CompactionDecision,
    CompactionRuntimeRecord, ContextCompactionStrategy, ContextTokenPressure,
};
use crate::services::api::Message;
#[cfg(test)]
use crate::services::api::ToolCall;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CompactionAttemptInput {
    pub trigger: String,
    pub strategy: ContextCompactionStrategy,
    pub decision: CompactionDecision,
    pub before_tokens: u64,
    pub after_tokens: Option<u64>,
    pub messages_before: usize,
    pub messages_after: Option<usize>,
    pub reason: String,
    pub boundary_id: Option<String>,
}

impl CompactionAttemptInput {
    pub fn new(
        trigger: impl Into<String>,
        strategy: ContextCompactionStrategy,
        decision: CompactionDecision,
        before_tokens: u64,
        messages_before: usize,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            trigger: trigger.into(),
            strategy,
            decision,
            before_tokens,
            after_tokens: None,
            messages_before,
            messages_after: None,
            reason: reason.into(),
            boundary_id: None,
        }
    }

    pub fn with_after(mut self, after_tokens: Option<u64>, messages_after: Option<usize>) -> Self {
        self.after_tokens = after_tokens;
        self.messages_after = messages_after;
        self
    }

    pub fn with_boundary_id(mut self, boundary_id: Option<String>) -> Self {
        self.boundary_id = boundary_id;
        self
    }
}

// ── 摘要模板 ──────────────────────────────────────────────

/// 结构化摘要的 8 段模板（Hermes 风格）
pub const SUMMARY_TEMPLATE: &str = "\
## Goal
{goal}

## Constraints
{constraints}

## Progress
{progress}

## Key Decisions
{decisions}

## Relevant Files
{files}

## Next Steps
{next_steps}

## Critical Context
{critical_context}

## Tools & Patterns
{tools}
";

/// 压缩摘要前缀（告知模型上下文已被压缩，避免重复工作）
/// 参考 Hermes: SUMMARY_PREFIX
pub const SUMMARY_PREFIX: &str = "\
[CONTEXT COMPACTION] Earlier turns in this conversation were compacted \
to save context space. The summary below describes work that was \
already completed, and the current session state may still reflect \
that work (for example, files may already be changed). Use the summary \
and the current state to continue from where things left off, and \
avoid repeating work:";

/// 会话记忆压缩策略（对标 Claude Code 的 sessionMemoryCompact）
///
/// 基于会话历史的智能压缩：
/// 1. 识别高频出现的文件/工具/模式，保留到 Critical Context
/// 2. 自动提取用户偏好（从记忆系统）
/// 3. 识别并保留未完成的任务链
#[derive(Debug, Clone, Default)]
pub struct SessionMemoryCompact {
    /// 从会话中提取的关键文件（出现频率高的）
    pub hot_files: Vec<String>,
    /// 用户偏好记忆（从 MemoryManager 注入）
    pub user_preferences: Vec<String>,
    /// 未完成的任务链
    pub pending_tasks: Vec<String>,
    /// 高频使用的工具模式
    pub tool_patterns: Vec<String>,
}

impl SessionMemoryCompact {
    /// 从消息历史中分析并提取会话记忆
    pub fn analyze(messages: &[Message]) -> Self {
        use std::collections::HashMap;

        let mut file_counts: HashMap<String, usize> = HashMap::new();
        let mut tool_counts: HashMap<String, usize> = HashMap::new();
        let mut pending: Vec<String> = Vec::new();

        for msg in messages {
            let text = msg.content();

            // 提取文件路径（简单启发式）
            for word in text.split_whitespace() {
                if word.contains('.') && (word.contains('/') || word.contains("\\")) {
                    *file_counts.entry(word.to_string()).or_insert(0) += 1;
                }
            }

            // 提取工具使用模式
            if text.contains("Tool: ") || text.contains("tool_call") {
                for line in text.lines() {
                    if let Some(tool) = line.strip_prefix("Tool: ") {
                        *tool_counts.entry(tool.to_string()).or_insert(0) += 1;
                    }
                }
            }

            // 提取未完成任务（TODO/FIXME/ pending）
            let lower = text.to_lowercase();
            if lower.contains("todo") || lower.contains("fixme") || lower.contains("pending") {
                for line in text.lines() {
                    let ll = line.to_lowercase();
                    if ll.contains("todo") || ll.contains("fixme") || ll.contains("pending") {
                        pending.push(line.trim().to_string());
                    }
                }
            }
        }

        // 取出现频率最高的文件（top 5）
        let mut hot_files: Vec<(String, usize)> = file_counts.into_iter().collect();
        hot_files.sort_by(|a, b| b.1.cmp(&a.1));

        // 取出现频率最高的工具模式（top 3）
        let mut tool_patterns: Vec<(String, usize)> = tool_counts.into_iter().collect();
        tool_patterns.sort_by(|a, b| b.1.cmp(&a.1));

        Self {
            hot_files: hot_files.into_iter().take(5).map(|(f, _)| f).collect(),
            user_preferences: Vec::new(), // 由外部注入
            pending_tasks: pending.into_iter().take(10).collect(),
            tool_patterns: tool_patterns.into_iter().take(3).map(|(t, _)| t).collect(),
        }
    }

    /// 将会话记忆注入到摘要文本中
    pub fn inject_into_summary(&self, summary: &mut String) {
        if !self.user_preferences.is_empty() {
            summary.push_str("\n\n## User Preferences\n");
            for preference in &self.user_preferences {
                summary.push_str(&format!("- {}\n", preference));
            }
        }
        if !self.hot_files.is_empty() {
            summary.push_str("\n\n## Frequently Accessed Files\n");
            for f in &self.hot_files {
                summary.push_str(&format!("- {}\n", f));
            }
        }
        if !self.pending_tasks.is_empty() {
            summary.push_str("\n## Pending Tasks\n");
            for t in &self.pending_tasks {
                summary.push_str(&format!("- {}\n", t));
            }
        }
        if !self.tool_patterns.is_empty() {
            summary.push_str("\n## Common Tool Patterns\n");
            for p in &self.tool_patterns {
                summary.push_str(&format!("- {}\n", p));
            }
        }
    }

    pub fn provenance_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        if !self.hot_files.is_empty() {
            tags.push(format!("session_memory:hot_files={}", self.hot_files.len()));
        }
        if !self.user_preferences.is_empty() {
            tags.push(format!(
                "session_memory:user_preferences={}",
                self.user_preferences.len()
            ));
        }
        if !self.pending_tasks.is_empty() {
            tags.push(format!(
                "session_memory:pending_tasks={}",
                self.pending_tasks.len()
            ));
        }
        if !self.tool_patterns.is_empty() {
            tags.push(format!(
                "session_memory:tool_patterns={}",
                self.tool_patterns.len()
            ));
        }
        tags
    }
}

/// Explicit runtime facts that make a compacted long task resumable.
///
/// This intentionally only keeps labeled state lines. Free-form conversation
/// can still be summarized heuristically, but continuation-critical facts must
/// be emitted with stable labels by runtime/tooling before they are promoted.
#[derive(Debug, Clone, Default)]
struct RuntimeContinuityFacts {
    active_objectives: Vec<String>,
    changed_files: Vec<String>,
    file_change_rounds: Vec<String>,
    validation_states: Vec<String>,
    terminal_task_states: Vec<String>,
    permission_states: Vec<String>,
    context_attachments: Vec<String>,
    diagnostic_states: Vec<String>,
    subagent_task_states: Vec<String>,
}

impl RuntimeContinuityFacts {
    fn analyze(messages: &[Message]) -> Self {
        let mut facts = Self::default();
        for msg in messages {
            for line in msg.content().lines() {
                facts.capture_line(line.trim());
            }
        }
        facts
    }

    fn capture_line(&mut self, line: &str) {
        let line = Self::normalize_line(line);
        if line.is_empty() {
            return;
        }

        let lower = line.to_lowercase();
        if Self::is_active_objective(&lower) {
            Self::push_unique(&mut self.active_objectives, &line, 5);
        }
        if Self::is_changed_files(&lower) {
            Self::push_unique(&mut self.changed_files, &line, 8);
        }
        if Self::is_file_change_round(&lower) {
            Self::push_unique(&mut self.file_change_rounds, &line, 8);
        }
        if Self::is_validation_state(&lower) {
            Self::push_unique(&mut self.validation_states, &line, 8);
        }
        if Self::is_terminal_task_state(&lower) {
            Self::push_unique(&mut self.terminal_task_states, &line, 8);
        }
        if Self::is_permission_state(&lower) {
            Self::push_unique(&mut self.permission_states, &line, 8);
        }
        if Self::is_context_attachment(&lower) {
            Self::push_unique(&mut self.context_attachments, &line, 8);
        }
        if Self::is_diagnostic_state(&lower) {
            Self::push_unique(&mut self.diagnostic_states, &line, 8);
        }
        if Self::is_subagent_task_state(&lower) {
            Self::push_unique(&mut self.subagent_task_states, &line, 8);
        }
    }

    fn normalize_line(line: &str) -> String {
        line.trim()
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim_start_matches("[ ] ")
            .trim_start_matches("[x] ")
            .chars()
            .take(240)
            .collect::<String>()
    }

    fn push_unique(target: &mut Vec<String>, line: &str, max: usize) {
        if target.len() >= max || target.iter().any(|item| item == line) {
            return;
        }
        target.push(line.to_string());
    }

    fn is_active_objective(lower: &str) -> bool {
        lower.starts_with("active objective:")
            || lower.starts_with("current objective:")
            || lower.starts_with("objective:")
    }

    fn is_changed_files(lower: &str) -> bool {
        lower.starts_with("changed files:")
            || lower.starts_with("changed file:")
            || lower.starts_with("modified files:")
            || lower.starts_with("files changed:")
    }

    fn is_file_change_round(lower: &str) -> bool {
        lower.starts_with("file change round:")
            || lower.starts_with("file-change round:")
            || lower.starts_with("tool round:")
            || lower.starts_with("tool-round:")
            || lower.contains("file_change_round")
            || (lower.contains("round_") && lower.contains("checkpoint"))
            || (lower.contains("tool round") && lower.contains("file"))
    }

    fn is_validation_state(lower: &str) -> bool {
        lower.starts_with("validation passed:")
            || lower.starts_with("validation failed:")
            || lower.starts_with("validation partial:")
            || lower.starts_with("required validation:")
            || lower.starts_with("verified:")
            || ((lower.contains("cargo test") || lower.contains("cargo check"))
                && (lower.contains("passed") || lower.contains("failed")))
    }

    fn is_terminal_task_state(lower: &str) -> bool {
        lower.starts_with("terminal task:")
            || lower.starts_with("terminal-task:")
            || lower.contains("terminal_task")
            || lower.contains("terminal task")
            || (lower.contains("task_id") && lower.contains("output_path"))
            || (lower.contains("shell_") && lower.contains("output"))
    }

    fn is_permission_state(lower: &str) -> bool {
        lower.starts_with("permission pending:")
            || lower.starts_with("permission requested:")
            || lower.starts_with("permission decision:")
            || lower.starts_with("permission state:")
            || lower.contains("permission_decision_evidence")
            || (lower.contains("permission") && lower.contains("risk_level"))
            || (lower.contains("permission") && lower.contains("matched_rules"))
    }

    fn is_context_attachment(lower: &str) -> bool {
        lower.starts_with("attached context:")
            || lower.starts_with("context attachment:")
            || lower.starts_with("run context:")
            || lower.contains("attached_context")
            || lower.contains("current_diff")
    }

    fn is_diagnostic_state(lower: &str) -> bool {
        lower.starts_with("diagnostics:")
            || lower.starts_with("diagnostic state:")
            || lower.starts_with("diagnostics delta:")
            || lower.contains("diagnostics_delta")
            || lower.contains("diagnostics after")
            || lower.contains("diagnostics before")
    }

    fn is_subagent_task_state(lower: &str) -> bool {
        lower.starts_with("active subagent:")
            || lower.starts_with("active sub-agent:")
            || lower.starts_with("subagent state:")
            || lower.starts_with("sub-agent state:")
            || lower.starts_with("agent task:")
            || (lower.contains("agent_id") && lower.contains("task_id"))
            || (lower.contains("subagent") && lower.contains("worktree"))
            || (lower.contains("sub-agent") && lower.contains("worktree"))
    }

    fn inject_into_summary(&self, summary: &mut String) {
        if self.is_empty() {
            return;
        }
        summary.push_str("\n\n## Runtime Continuity\n");
        Self::append_group(summary, "Active objectives", &self.active_objectives);
        Self::append_group(summary, "Changed files", &self.changed_files);
        Self::append_group(summary, "File-change rounds", &self.file_change_rounds);
        Self::append_group(summary, "Validation state", &self.validation_states);
        Self::append_group(summary, "Terminal task state", &self.terminal_task_states);
        Self::append_group(summary, "Permission state", &self.permission_states);
        Self::append_group(summary, "Attached context", &self.context_attachments);
        Self::append_group(summary, "Diagnostics state", &self.diagnostic_states);
        Self::append_group(summary, "Subagent/task state", &self.subagent_task_states);
    }

    fn append_group(summary: &mut String, label: &str, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        summary.push_str(&format!("{}:\n", label));
        for line in lines {
            summary.push_str(&format!("- {}\n", line));
        }
    }

    fn is_empty(&self) -> bool {
        self.active_objectives.is_empty()
            && self.changed_files.is_empty()
            && self.file_change_rounds.is_empty()
            && self.validation_states.is_empty()
            && self.terminal_task_states.is_empty()
            && self.permission_states.is_empty()
            && self.context_attachments.is_empty()
            && self.diagnostic_states.is_empty()
            && self.subagent_task_states.is_empty()
    }

    fn retained_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        if !self.active_objectives.is_empty() {
            items.push(format!(
                "runtime_state_active_objectives:{}",
                self.active_objectives.len()
            ));
        }
        if !self.changed_files.is_empty() {
            items.push(format!(
                "runtime_state_changed_files:{}",
                self.changed_files.len()
            ));
        }
        if !self.validation_states.is_empty() {
            items.push(format!(
                "runtime_state_validation:{}",
                self.validation_states.len()
            ));
        }
        if !self.file_change_rounds.is_empty() {
            items.push(format!(
                "runtime_state_file_change_rounds:{}",
                self.file_change_rounds.len()
            ));
        }
        if !self.terminal_task_states.is_empty() {
            items.push(format!(
                "runtime_state_terminal_tasks:{}",
                self.terminal_task_states.len()
            ));
        }
        if !self.permission_states.is_empty() {
            items.push(format!(
                "runtime_state_permissions:{}",
                self.permission_states.len()
            ));
        }
        if !self.context_attachments.is_empty() {
            items.push(format!(
                "runtime_state_context_attachments:{}",
                self.context_attachments.len()
            ));
        }
        if !self.diagnostic_states.is_empty() {
            items.push(format!(
                "runtime_state_diagnostics:{}",
                self.diagnostic_states.len()
            ));
        }
        if !self.subagent_task_states.is_empty() {
            items.push(format!(
                "runtime_state_subagent_tasks:{}",
                self.subagent_task_states.len()
            ));
        }
        items
    }

    fn provenance_tags(&self) -> Vec<String> {
        self.retained_items()
            .into_iter()
            .map(|item| format!("runtime_continuity:{}", item))
            .collect()
    }
}

/// 给 LLM 的压缩 prompt 模板
pub const COMPRESSION_PROMPT_TEMPLATE: &str = "\
You are a conversation compressor. Summarize the following conversation into \
a structured format. Be concise but preserve ALL critical information.

You MUST use exactly these 8 sections (even if empty, include the header):

## Goal
[What is the user trying to accomplish? One sentence.]

## Constraints
[Known constraints, limitations, or requirements discovered so far.]

## Progress
- [Completed items]
- [In-progress items]
- [Blocked items, if any]

## Key Decisions
- [Decision made and reason]

## Relevant Files
- [file_path: what was done]

## Next Steps
- [Immediate next actions]

## Critical Context
- [Information that MUST NOT be lost - API keys locations, specific error messages, 
   architectural decisions, user preferences stated]

## Tools & Patterns
- [Tool usage patterns that worked: e.g., 'grep before edit', 'test after each change']

Conversation to summarize:
{conversation}
";

// ── Token 预算 ────────────────────────────────────────────

/// Token 预算
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// 模型最大上下文长度
    pub max_context_tokens: u64,
    /// 保留给输出的 token 数
    pub reserved_output_tokens: u64,
    /// 系统 prompt 预估 token 数
    pub system_prompt_tokens: u64,
    /// 工具 schema 预估 token 数
    pub tool_schemas_tokens: u64,
}

/// 时间基础压缩配置（新增）
#[derive(Debug, Clone)]
pub struct TimeBasedConfig {
    /// 会话时长阈值（秒），超过此值即使 token 充裕也触发压缩
    pub session_duration_threshold_secs: u64,
    /// 消息数量阈值，超过此值触发压缩
    pub message_count_threshold: usize,
    /// 空闲阈值（秒），超过此值后触发微压缩
    pub idle_threshold_secs: u64,
    /// 是否启用时间基础压缩
    pub enabled: bool,
}

impl Default for TimeBasedConfig {
    fn default() -> Self {
        Self {
            session_duration_threshold_secs: std::env::var(
                "PRIORITY_AGENT_SESSION_DURATION_THRESHOLD",
            )
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3600), // 默认 1 小时
            message_count_threshold: std::env::var("PRIORITY_AGENT_MESSAGE_COUNT_THRESHOLD")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(100),
            idle_threshold_secs: std::env::var("PRIORITY_AGENT_IDLE_THRESHOLD")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(300), // 默认 5 分钟
            enabled: std::env::var("PRIORITY_AGENT_TIME_BASED_COMPRESSION")
                .map(|v| v != "false")
                .unwrap_or(true),
        }
    }
}

/// 压缩级别（分层压缩流水线）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    /// 不压缩
    None,
    /// 轻量：只裁剪旧工具输出（最快，零 LLM 调用）
    Light,
    /// 中等：裁剪 + 启发式摘要（快速，不依赖 LLM）
    Medium,
    /// 重度：裁剪 + LLM 摘要（最高质量，但有延迟和成本）
    Heavy,
}

impl CompressionLevel {
    /// 根据 token 使用率和历史自动选择压缩级别
    pub fn auto_select(
        usage_ratio: f64,
        compression_count: u32,
        consecutive_llm_failures: u32,
        has_llm_provider: bool,
    ) -> Self {
        if usage_ratio < 0.7 {
            CompressionLevel::Light
        } else if usage_ratio < 0.85 {
            // 中等负载：如果有 LLM 且未连续失败，用 Medium；否则 Light
            if has_llm_provider && consecutive_llm_failures < 2 {
                CompressionLevel::Medium
            } else {
                CompressionLevel::Light
            }
        } else {
            // 高负载：必须压缩
            if has_llm_provider && consecutive_llm_failures < 3 {
                if compression_count < 2 {
                    CompressionLevel::Heavy
                } else {
                    CompressionLevel::Medium
                }
            } else {
                CompressionLevel::Medium
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CompressionLevel::None => "none",
            CompressionLevel::Light => "light",
            CompressionLevel::Medium => "medium",
            CompressionLevel::Heavy => "heavy",
        }
    }
}

/// 压缩警告状态（新增）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionWarning {
    /// 正常，无警告
    None,
    /// 接近阈值（>60%）
    Approaching,
    /// 快满了（>80%）
    Near,
    /// 即将压缩（>90%）
    Critical,
}

impl CompressionWarning {
    /// 根据 token 使用率计算警告级别
    pub fn from_usage_ratio(ratio: f64) -> Self {
        if ratio > 0.9 {
            CompressionWarning::Critical
        } else if ratio > 0.8 {
            CompressionWarning::Near
        } else if ratio > 0.6 {
            CompressionWarning::Approaching
        } else {
            CompressionWarning::None
        }
    }

    /// 获取用户友好的提示文本
    pub fn message(&self) -> &'static str {
        match self {
            CompressionWarning::None => "",
            CompressionWarning::Approaching => {
                "Context usage is approaching 60%. Consider wrapping up soon."
            }
            CompressionWarning::Near => "Context is 80% full. Compression will happen soon.",
            CompressionWarning::Critical => "Context is nearly full! Compression imminent.",
        }
    }
}

impl TokenBudget {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            max_context_tokens,
            reserved_output_tokens: 4096,
            system_prompt_tokens: 2000,
            tool_schemas_tokens: 1000,
        }
    }

    pub fn from_model_context_profile(
        profile: &crate::engine::model_context::ModelContextProfile,
    ) -> Self {
        Self {
            max_context_tokens: profile.context_window_tokens,
            reserved_output_tokens: profile.reserved_output_tokens,
            system_prompt_tokens: 2000,
            tool_schemas_tokens: 1000,
        }
    }

    /// 可用于对话历史的 token 数
    pub fn available_for_history(&self) -> u64 {
        self.max_context_tokens
            .saturating_sub(self.reserved_output_tokens)
            .saturating_sub(self.system_prompt_tokens)
            .saturating_sub(self.tool_schemas_tokens)
    }

    /// 是否需要压缩（历史超过可用空间的 80%）
    pub fn needs_compression(&self, estimated_tokens: u64) -> bool {
        let threshold = self.available_for_history() * 80 / 100;
        estimated_tokens > threshold
    }

    /// 目标压缩大小（保留最近的 60%）
    pub fn target_tokens(&self) -> u64 {
        self.available_for_history() * 60 / 100
    }

    /// 尾部保护的 soft ceiling（1.5x budget，防止超大消息中间切割）
    pub fn tail_soft_ceiling(&self) -> u64 {
        self.target_tokens() * 150 / 100
    }
}

fn compaction_retained_items(
    head_count: usize,
    tail_count: usize,
    compact_meta: Option<&CompactMetadata>,
    session_memory: &SessionMemoryCompact,
    runtime_continuity: &RuntimeContinuityFacts,
) -> Vec<String> {
    let mut items = vec![
        format!("head_messages:{}", head_count),
        format!("tail_messages:{}", tail_count),
        "recent_tool_results:last_3".to_string(),
        "tool_call_pairs:sanitized".to_string(),
    ];
    if let Some(meta) = compact_meta {
        items.push(format!("compact_boundary:{}", meta.boundary_id));
    }
    if !session_memory.hot_files.is_empty() {
        items.push(format!(
            "session_memory_hot_files:{}",
            session_memory.hot_files.len()
        ));
    }
    if !session_memory.pending_tasks.is_empty() {
        items.push(format!(
            "session_memory_pending_tasks:{}",
            session_memory.pending_tasks.len()
        ));
    }
    if !session_memory.tool_patterns.is_empty() {
        items.push(format!(
            "session_memory_tool_patterns:{}",
            session_memory.tool_patterns.len()
        ));
    }
    if !session_memory.user_preferences.is_empty() {
        items.push(format!(
            "session_memory_user_preferences:{}",
            session_memory.user_preferences.len()
        ));
    }
    items.extend(runtime_continuity.retained_items());
    items
}

fn compaction_token_delta(tokens_before: u64, tokens_after: u64) -> i64 {
    i64::try_from(tokens_after).unwrap_or(i64::MAX)
        - i64::try_from(tokens_before).unwrap_or(i64::MAX)
}

fn compaction_stage_order(strategy: ContextCompactionStrategy) -> Vec<String> {
    match strategy {
        ContextCompactionStrategy::Snip => vec!["snip_tool_results"],
        ContextCompactionStrategy::MicroCompact => vec!["snip_tool_results", "sanitize_tool_pairs"],
        ContextCompactionStrategy::AutoCompact
        | ContextCompactionStrategy::ReactiveCompact
        | ContextCompactionStrategy::SessionMemoryCompact => vec![
            "snip_tool_results",
            "split_head",
            "align_boundary_forward",
            "split_tail",
            "summarize_or_merge",
            "restore_runtime_continuity",
            "embed_compact_boundary",
            "sanitize_tool_pairs",
        ],
        ContextCompactionStrategy::NoOp => vec!["no_op"],
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

// ── Token 估算 ────────────────────────────────────────────

/// 简单 token 估算（4 字符 ≈ 1 token）
pub fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64).div_ceil(4)
}

/// 估算消息列表的总 token 数
pub fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|m| {
            let content_tokens = estimate_tokens(&m.content());
            let overhead = 4; // role, formatting 等开销
            content_tokens + overhead
        })
        .sum()
}

/// 估算工具 schema 的 token 数
pub fn estimate_tool_schemas_tokens(tools: &[crate::services::api::Tool]) -> u64 {
    tools
        .iter()
        .map(|t| {
            estimate_tokens(&t.name)
                + estimate_tokens(&t.description)
                + estimate_tokens(&serde_json::to_string(&t.parameters).unwrap_or_default())
                + 10
        })
        .sum()
}

impl Message {
    /// 获取消息内容（用于 token 估算）
    fn content(&self) -> String {
        match self {
            Message::System { content } => content.clone(),
            Message::User { content } => content.clone(),
            Message::Assistant { content, .. } => content.clone(),
            Message::Tool { content, .. } => content.clone(),
        }
    }
}

// ── 8 段结构化摘要 ────────────────────────────────────────

/// 结构化摘要（Hermes 8 段模板）
#[derive(Debug, Clone)]
pub struct StructuredSummary {
    pub goal: String,
    pub constraints: Vec<String>,
    pub progress_done: Vec<String>,
    pub progress_in_progress: Vec<String>,
    pub progress_blocked: Vec<String>,
    pub decisions: Vec<String>,
    pub files_modified: Vec<String>,
    pub next_steps: Vec<String>,
    pub critical_context: Vec<String>,
    pub tools_patterns: Vec<String>,
}

impl StructuredSummary {
    pub fn new() -> Self {
        Self {
            goal: String::new(),
            constraints: Vec::new(),
            progress_done: Vec::new(),
            progress_in_progress: Vec::new(),
            progress_blocked: Vec::new(),
            decisions: Vec::new(),
            files_modified: Vec::new(),
            next_steps: Vec::new(),
            critical_context: Vec::new(),
            tools_patterns: Vec::new(),
        }
    }

    /// 兼容旧 API — 获取所有 progress 合并
    pub fn progress_all(&self) -> Vec<String> {
        let mut all = Vec::new();
        for p in &self.progress_done {
            all.push(format!("[Done] {}", p));
        }
        for p in &self.progress_in_progress {
            all.push(format!("[In Progress] {}", p));
        }
        for p in &self.progress_blocked {
            all.push(format!("[Blocked] {}", p));
        }
        all
    }

    /// 从 LLM 摘要文本解析
    pub fn from_text(text: &str) -> Self {
        let mut summary = Self::new();
        let mut current_section = "";
        let mut progress_subsection = "";

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("## ") || line.starts_with("# ") {
                let section = line.trim_start_matches('#').trim();
                // 检查 Progress 子节
                if section.contains("Done") || section.contains("完成") {
                    progress_subsection = "done";
                    continue;
                } else if section.contains("In Progress") || section.contains("进行中") {
                    progress_subsection = "in_progress";
                    continue;
                } else if section.contains("Blocked") || section.contains("阻塞") {
                    progress_subsection = "blocked";
                    continue;
                }
                current_section = section;
                progress_subsection = "";
                continue;
            }

            // 处理子节（如 "- [Done] xxx"）
            if !line.is_empty() && !line.starts_with("```") {
                let content = line
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim_start_matches("[ ] ")
                    .trim_start_matches("[x] ");

                // 检测行内标签
                let actual_content = if content.starts_with("[Done]") {
                    progress_subsection = "done";
                    content.trim_start_matches("[Done]").trim()
                } else if content.starts_with("[In Progress]") {
                    progress_subsection = "in_progress";
                    content.trim_start_matches("[In Progress]").trim()
                } else if content.starts_with("[Blocked]") {
                    progress_subsection = "blocked";
                    content.trim_start_matches("[Blocked]").trim()
                } else {
                    content
                };

                if actual_content.is_empty() {
                    continue;
                }

                match current_section {
                    s if s.contains("Goal") || s.contains("目标") => {
                        if summary.goal.is_empty() {
                            summary.goal = actual_content.to_string();
                        }
                    }
                    s if s.contains("Constraint") || s.contains("约束") || s.contains("限制") =>
                    {
                        summary.constraints.push(actual_content.to_string());
                    }
                    s if s.contains("Progress") || s.contains("进展") => {
                        match progress_subsection {
                            "done" => summary.progress_done.push(actual_content.to_string()),
                            "in_progress" => summary
                                .progress_in_progress
                                .push(actual_content.to_string()),
                            "blocked" => summary.progress_blocked.push(actual_content.to_string()),
                            _ => summary
                                .progress_in_progress
                                .push(actual_content.to_string()),
                        }
                    }
                    s if s.contains("Decision") || s.contains("决策") => {
                        summary.decisions.push(actual_content.to_string());
                    }
                    s if s.contains("File") || s.contains("文件") || s.contains("Relevant") => {
                        summary.files_modified.push(actual_content.to_string());
                    }
                    s if s.contains("Next") || s.contains("下一步") => {
                        summary.next_steps.push(actual_content.to_string());
                    }
                    s if s.contains("Critical") || s.contains("关键") => {
                        summary.critical_context.push(actual_content.to_string());
                    }
                    s if s.contains("Tool") || s.contains("Pattern") || s.contains("工具") => {
                        summary.tools_patterns.push(actual_content.to_string());
                    }
                    _ => {}
                }
            }
        }

        summary
    }

    /// 转为文本（完整 8 段格式，Hermes 风格）
    pub fn to_text(&self) -> String {
        let mut text = String::new();

        if !self.goal.is_empty() {
            text.push_str(&format!("## Goal\n{}\n\n", self.goal));
        }

        if !self.constraints.is_empty() {
            text.push_str("## Constraints & Preferences\n");
            for c in &self.constraints {
                text.push_str(&format!("- {}\n", c));
            }
            text.push('\n');
        }

        // Progress 分为 Done / In Progress / Blocked 三个子节
        if !self.progress_done.is_empty()
            || !self.progress_in_progress.is_empty()
            || !self.progress_blocked.is_empty()
        {
            text.push_str("## Progress\n");
            if !self.progress_done.is_empty() {
                text.push_str("\nDone:\n");
                for p in &self.progress_done {
                    text.push_str(&format!("- {}\n", p));
                }
            }
            if !self.progress_in_progress.is_empty() {
                text.push_str("\nIn Progress:\n");
                for p in &self.progress_in_progress {
                    text.push_str(&format!("- {}\n", p));
                }
            }
            if !self.progress_blocked.is_empty() {
                text.push_str("\nBlocked:\n");
                for p in &self.progress_blocked {
                    text.push_str(&format!("- {}\n", p));
                }
            }
            text.push('\n');
        }

        if !self.decisions.is_empty() {
            text.push_str("## Key Decisions\n");
            for d in &self.decisions {
                text.push_str(&format!("- {}\n", d));
            }
            text.push('\n');
        }

        if !self.files_modified.is_empty() {
            text.push_str("## Relevant Files\n");
            for f in &self.files_modified {
                text.push_str(&format!("- {}\n", f));
            }
            text.push('\n');
        }

        if !self.next_steps.is_empty() {
            text.push_str("## Next Steps\n");
            for n in &self.next_steps {
                text.push_str(&format!("- {}\n", n));
            }
            text.push('\n');
        }

        if !self.critical_context.is_empty() {
            text.push_str("## Critical Context\n");
            for c in &self.critical_context {
                text.push_str(&format!("- {}\n", c));
            }
            text.push('\n');
        }

        if !self.tools_patterns.is_empty() {
            text.push_str("## Tools & Patterns\n");
            for t in &self.tools_patterns {
                text.push_str(&format!("- {}\n", t));
            }
        }

        text
    }

    /// 合并新摘要（迭代更新，累积知识）
    /// - goal / next_steps / constraints: 用最新的
    /// - progress / decisions / files / critical_context / tools: 累积去重
    pub fn merge(&mut self, new: &StructuredSummary) {
        if !new.goal.is_empty() {
            self.goal = new.goal.clone();
        }
        if !new.constraints.is_empty() {
            for c in &new.constraints {
                if !self.constraints.contains(c) {
                    self.constraints.push(c.clone());
                }
            }
        }
        for p in &new.progress_done {
            if !self.progress_done.contains(p) {
                self.progress_done.push(p.clone());
            }
        }
        for p in &new.progress_in_progress {
            if !self.progress_in_progress.contains(p) {
                self.progress_in_progress.push(p.clone());
            }
        }
        for p in &new.progress_blocked {
            if !self.progress_blocked.contains(p) {
                self.progress_blocked.push(p.clone());
            }
        }
        for d in &new.decisions {
            if !self.decisions.contains(d) {
                self.decisions.push(d.clone());
            }
        }
        for f in &new.files_modified {
            if !self.files_modified.contains(f) {
                self.files_modified.push(f.clone());
            }
        }
        if !new.next_steps.is_empty() {
            self.next_steps = new.next_steps.clone();
        }
        for c in &new.critical_context {
            if !self.critical_context.contains(c) {
                self.critical_context.push(c.clone());
            }
        }
        for t in &new.tools_patterns {
            if !self.tools_patterns.contains(t) {
                self.tools_patterns.push(t.clone());
            }
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.goal.is_empty()
            && self.constraints.is_empty()
            && self.progress_done.is_empty()
            && self.progress_in_progress.is_empty()
            && self.progress_blocked.is_empty()
            && self.decisions.is_empty()
            && self.files_modified.is_empty()
            && self.next_steps.is_empty()
            && self.critical_context.is_empty()
            && self.tools_patterns.is_empty()
    }
}

impl Default for StructuredSummary {
    fn default() -> Self {
        Self::new()
    }
}

// ── 上下文压缩器 ──────────────────────────────────────────

/// 上下文压缩器
pub struct ContextCompressor {
    budget: TokenBudget,
    /// 时间基础配置（新增）
    time_config: TimeBasedConfig,
    /// 会话开始时间
    session_start: std::time::Instant,
    /// 累积的摘要（跨多次压缩保持 — 迭代式摘要）
    accumulated_summary: Option<StructuredSummary>,
    /// 压缩次数
    compression_count: u32,
    /// 上次压缩失败时间（用于冷却期）
    last_failure_time: Option<std::time::Instant>,
    /// 冷却期（秒）
    cooldown_secs: u64,
    /// LLM Provider（可选，用于生成高质量摘要）
    llm_provider: Option<std::sync::Arc<dyn crate::services::api::LlmProvider>>,
    /// LLM 摘要用的模型名
    llm_model: String,
    /// 压缩前总 token 数（累积）
    total_tokens_before: u64,
    /// 压缩后总 token 数（累积）
    total_tokens_after: u64,
    /// LLM 压缩尝试次数
    llm_compression_attempts: u32,
    /// LLM 压缩失败次数
    llm_compression_failures: u32,
    /// 连续 LLM 压缩失败次数（用于快速熔断）
    consecutive_llm_failures: u32,
    /// 连续失败熔断阈值
    max_consecutive_llm_failures: u32,
    /// Compact Boundary 序列号（单调递增）
    compact_sequence: u32,
    /// Compact Boundary 历史（用于追踪和恢复）
    compact_metadata_history: Vec<CompactMetadata>,
    /// Runtime compaction records for trace/UI provenance.
    compaction_records: Vec<CompactionRuntimeRecord>,
    /// State-machine records for every compaction decision, including skips.
    compaction_attempt_records: Vec<CompactionAttemptRecord>,
    consecutive_compaction_failures: u32,
    consecutive_no_gain_compactions: u32,
    max_consecutive_compaction_failures: u32,
    max_consecutive_no_gain_compactions: u32,
}

impl ContextCompressor {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            budget: TokenBudget::new(max_context_tokens),
            time_config: TimeBasedConfig::default(),
            session_start: std::time::Instant::now(),
            accumulated_summary: None,
            compression_count: 0,
            last_failure_time: None,
            cooldown_secs: 600, // 10 分钟冷却
            llm_provider: None,
            llm_model: String::new(),
            total_tokens_before: 0,
            total_tokens_after: 0,
            llm_compression_attempts: 0,
            llm_compression_failures: 0,
            consecutive_llm_failures: 0,
            max_consecutive_llm_failures: 3,
            compact_sequence: 0,
            compact_metadata_history: Vec::new(),
            compaction_records: Vec::new(),
            compaction_attempt_records: Vec::new(),
            consecutive_compaction_failures: 0,
            consecutive_no_gain_compactions: 0,
            max_consecutive_compaction_failures: 2,
            max_consecutive_no_gain_compactions: 2,
        }
    }

    pub fn from_model_context_profile(
        profile: &crate::engine::model_context::ModelContextProfile,
    ) -> Self {
        Self {
            budget: TokenBudget::from_model_context_profile(profile),
            ..Self::new(profile.context_window_tokens)
        }
    }

    /// 获取当前压缩警告级别
    pub fn warning_level(&self, messages: &[Message]) -> CompressionWarning {
        let tokens = estimate_messages_tokens(messages);
        let total = tokens + self.budget.system_prompt_tokens + self.budget.tool_schemas_tokens;
        let ratio = total as f64 / self.budget.max_context_tokens as f64;
        CompressionWarning::from_usage_ratio(ratio)
    }

    fn token_pressure_for_tokens(&self, message_tokens: u64) -> ContextTokenPressure {
        let total = message_tokens
            .saturating_add(self.budget.system_prompt_tokens)
            .saturating_add(self.budget.tool_schemas_tokens);
        let ratio = if self.budget.max_context_tokens == 0 {
            1.0
        } else {
            total as f64 / self.budget.max_context_tokens as f64
        };
        ContextTokenPressure::from_usage_ratio(ratio)
    }

    /// 检查是否需要基于时间的压缩
    pub fn needs_time_based_compression(&self, messages: &[Message]) -> bool {
        if !self.time_config.enabled {
            return false;
        }
        let elapsed = self.session_start.elapsed().as_secs();
        let msg_count = messages.len();

        elapsed > self.time_config.session_duration_threshold_secs
            || msg_count > self.time_config.message_count_threshold
    }

    /// 微压缩：轻量级压缩，不触发 LLM，仅裁剪工具输出
    /// 用于中等长度对话或空闲后轻量整理
    pub fn micro_compress(&mut self, messages: &[Message]) -> Vec<Message> {
        self.micro_compress_with_strategy(
            messages,
            ContextCompactionStrategy::MicroCompact,
            Some(CompressionLevel::Light),
        )
    }

    /// Snip old tool outputs without summarizing the conversation.
    pub fn snip_tool_results(&mut self, messages: &[Message]) -> Vec<Message> {
        let tokens_before = estimate_messages_tokens(messages);
        self.total_tokens_before += tokens_before;

        let result = Self::prune_old_tool_results(messages);
        let tokens_after = estimate_messages_tokens(&result);
        self.total_tokens_after += tokens_after;
        self.record_compaction(CompactionRuntimeRecord {
            strategy: ContextCompactionStrategy::Snip,
            level: None,
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
            messages_before: messages.len(),
            messages_after: result.len(),
            tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(tokens_before, tokens_after),
            stage_order: compaction_stage_order(ContextCompactionStrategy::Snip),
            boundary_id: None,
            sequence: None,
            preserved_tail_count: None,
            retained_items: vec!["recent_tool_results:last_3".to_string()],
            provenance: vec!["tool_result_snip".to_string()],
        });

        result
    }

    fn micro_compress_with_strategy(
        &mut self,
        messages: &[Message],
        strategy: ContextCompactionStrategy,
        level: Option<CompressionLevel>,
    ) -> Vec<Message> {
        let tokens_before = estimate_messages_tokens(messages);
        self.total_tokens_before += tokens_before;

        // 只做 Phase 0（裁剪旧工具输出）和 Phase 5（工具对校验）
        let pruned = Self::prune_old_tool_results(messages);
        let result = Self::sanitize_tool_pairs(pruned);

        let tokens_after = estimate_messages_tokens(&result);
        self.total_tokens_after += tokens_after;
        self.record_compaction(CompactionRuntimeRecord {
            strategy,
            level: level.map(|value| value.label().to_string()),
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
            messages_before: messages.len(),
            messages_after: result.len(),
            tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(tokens_before, tokens_after),
            stage_order: compaction_stage_order(strategy),
            boundary_id: None,
            sequence: None,
            preserved_tail_count: None,
            retained_items: vec![
                "recent_tool_results:last_3".to_string(),
                "tool_call_pairs:sanitized".to_string(),
            ],
            provenance: vec![
                "tool_result_snip".to_string(),
                "tool_pair_sanitize".to_string(),
            ],
        });

        info!(
            "Micro compression: {} messages -> {} messages ({} -> {} tokens)",
            messages.len(),
            result.len(),
            tokens_before,
            tokens_after
        );
        result
    }

    /// 设置系统 prompt 预估大小
    pub fn with_system_prompt_tokens(mut self, tokens: u64) -> Self {
        self.budget.system_prompt_tokens = tokens;
        self
    }

    /// 设置工具 schema 预估大小
    pub fn with_tool_schemas_tokens(mut self, tokens: u64) -> Self {
        self.budget.tool_schemas_tokens = tokens;
        self
    }

    /// 设置 LLM Provider（用于高质量摘要生成）
    pub fn with_llm_provider(
        mut self,
        provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
        model: impl Into<String>,
    ) -> Self {
        self.llm_provider = Some(provider);
        self.llm_model = model.into();
        self
    }

    /// 检查是否在冷却期（压缩失败后）
    pub fn is_in_cooldown(&self) -> bool {
        if let Some(last_failure) = self.last_failure_time {
            last_failure.elapsed().as_secs() < self.cooldown_secs
        } else {
            false
        }
    }

    /// 前置检查：是否需要压缩（包括系统提示和工具 schema）
    pub fn preflight_check(
        &self,
        messages: &[Message],
        system_prompt_tokens: u64,
        tool_schemas_tokens: u64,
    ) -> bool {
        if self.is_in_cooldown() {
            return false;
        }
        let total = estimate_messages_tokens(messages) + system_prompt_tokens + tool_schemas_tokens;
        let threshold = self.budget.max_context_tokens * 80 / 100;
        total > threshold
    }

    /// 检查是否需要压缩
    pub fn needs_compression(&self, messages: &[Message]) -> bool {
        if self.is_in_cooldown() {
            return false;
        }
        let tokens = estimate_messages_tokens(messages);
        self.budget.needs_compression(tokens)
    }

    /// 按级别压缩消息列表
    pub fn compress_with_level(
        &mut self,
        messages: &[Message],
        level: CompressionLevel,
    ) -> Vec<Message> {
        self.compress_with_level_for_strategy(
            messages,
            level,
            ContextCompactionStrategy::AutoCompact,
        )
    }

    fn compress_with_level_for_strategy(
        &mut self,
        messages: &[Message],
        level: CompressionLevel,
        strategy: ContextCompactionStrategy,
    ) -> Vec<Message> {
        let tokens_before = estimate_messages_tokens(messages);

        match level {
            CompressionLevel::None => {
                self.record_compaction(CompactionRuntimeRecord {
                    strategy,
                    level: Some(level.label().to_string()),
                    trigger: None,
                    token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
                    messages_before: messages.len(),
                    messages_after: messages.len(),
                    tokens_before,
                    tokens_after: tokens_before,
                    token_delta: 0,
                    stage_order: compaction_stage_order(ContextCompactionStrategy::NoOp),
                    boundary_id: None,
                    sequence: None,
                    preserved_tail_count: None,
                    retained_items: vec!["messages:all".to_string()],
                    provenance: vec!["level:none".to_string()],
                });
                messages.to_vec()
            }
            CompressionLevel::Light => {
                let r = self.micro_compress_with_strategy(messages, strategy, Some(level));
                let tokens_after = estimate_messages_tokens(&r);
                info!(
                    "Light compression ({}): {} -> {} tokens",
                    level.label(),
                    tokens_before,
                    tokens_after
                );
                r
            }
            CompressionLevel::Medium => {
                let r =
                    self.compress_with_summary_for_strategy(messages, None, strategy, Some(level));
                let tokens_after = estimate_messages_tokens(&r);
                info!(
                    "Medium compression ({}): {} -> {} tokens",
                    level.label(),
                    tokens_before,
                    tokens_after
                );
                r
            }
            CompressionLevel::Heavy => {
                // Heavy 需要 LLM，在 compress_async 中处理
                self.compress_with_summary_for_strategy(messages, None, strategy, Some(level))
            }
        }
    }

    /// 异步压缩消息列表（分层压缩流水线）
    /// 根据 token 使用率自动选择压缩级别：
    /// - Light (<70%): 只裁剪工具输出
    /// - Medium (70-85%): 裁剪 + 启发式摘要
    /// - Heavy (>85%): 裁剪 + LLM 摘要
    pub async fn compress_async(&mut self, messages: &[Message]) -> Vec<Message> {
        self.compress_async_with_strategy(messages, ContextCompactionStrategy::AutoCompact)
            .await
    }

    pub async fn compress_async_with_strategy(
        &mut self,
        messages: &[Message],
        strategy: ContextCompactionStrategy,
    ) -> Vec<Message> {
        let tokens_before = estimate_messages_tokens(messages);
        let total =
            tokens_before + self.budget.system_prompt_tokens + self.budget.tool_schemas_tokens;
        let usage_ratio = total as f64 / self.budget.max_context_tokens as f64;

        let level = CompressionLevel::auto_select(
            usage_ratio,
            self.compression_count,
            self.consecutive_llm_failures,
            self.has_llm_provider(),
        );

        debug!(
            "Compression auto-selected level={} (usage={:.1}%, count={}, llm_failures={})",
            level.label(),
            usage_ratio * 100.0,
            self.compression_count,
            self.consecutive_llm_failures
        );

        // Light/Medium 不需要 LLM，直接同步处理
        if level == CompressionLevel::Light || level == CompressionLevel::Medium {
            return self.compress_with_level_for_strategy(messages, level, strategy);
        }

        // Heavy: 尝试 LLM 摘要
        if self.has_llm_provider()
            && !self.is_in_cooldown()
            && self.consecutive_llm_failures < self.max_consecutive_llm_failures
        {
            self.llm_compression_attempts += 1;
            match self.llm_summarize_middle(messages).await {
                Some(summary_text) => {
                    self.consecutive_llm_failures = 0;
                    let compressed = self.compress_with_summary_for_strategy(
                        messages,
                        Some(&summary_text),
                        strategy,
                        Some(level),
                    );
                    let tokens_after = estimate_messages_tokens(&compressed);
                    info!(
                        "Heavy (LLM) compression succeeded: {} -> {} tokens (saved {}%)",
                        tokens_before,
                        tokens_after,
                        if tokens_before > 0 {
                            (tokens_before - tokens_after) * 100 / tokens_before
                        } else {
                            0
                        }
                    );
                    compressed
                }
                None => {
                    self.llm_compression_failures += 1;
                    self.consecutive_llm_failures += 1;
                    self.record_failure();
                    let compressed = self.compress_with_summary_for_strategy(
                        messages,
                        None,
                        strategy,
                        Some(level),
                    );
                    let tokens_after = estimate_messages_tokens(&compressed);
                    warn!(
                        "LLM compression failed, fell back to medium: {} -> {} tokens",
                        tokens_before, tokens_after
                    );
                    compressed
                }
            }
        } else {
            if self.consecutive_llm_failures >= self.max_consecutive_llm_failures {
                warn!(
                    "LLM compression disabled after {} consecutive failures; using medium compression.",
                    self.consecutive_llm_failures
                );
            }
            self.compress_with_summary_for_strategy(messages, None, strategy, Some(level))
        }
    }

    /// 压缩消息列表
    /// 返回压缩后的消息列表
    pub fn compress(&mut self, messages: &[Message]) -> Vec<Message> {
        self.compress_with_summary(messages, None)
    }

    /// 使用预计算的摘要文本压缩（同步）
    /// summary_text: Some(text) = 使用 LLM 生成的摘要; None = 使用启发式
    pub fn compress_with_summary(
        &mut self,
        messages: &[Message],
        summary_text: Option<&str>,
    ) -> Vec<Message> {
        self.compress_with_summary_for_strategy(
            messages,
            summary_text,
            ContextCompactionStrategy::AutoCompact,
            None,
        )
    }

    fn compress_with_summary_for_strategy(
        &mut self,
        messages: &[Message],
        summary_text: Option<&str>,
        strategy: ContextCompactionStrategy,
        level: Option<CompressionLevel>,
    ) -> Vec<Message> {
        let original_message_count = messages.len();
        let original_tokens_before = estimate_messages_tokens(messages);
        let summary_source_tag = if summary_text.is_some() {
            "summary_source:llm"
        } else {
            "summary_source:heuristic"
        };
        if messages.is_empty() {
            return messages.to_vec();
        }
        self.total_tokens_before += original_tokens_before;
        let session_memory = SessionMemoryCompact::analyze(messages);
        let runtime_continuity = RuntimeContinuityFacts::analyze(messages);

        info!(
            "Compressing {} messages (budget: {} available tokens, iteration: {})",
            messages.len(),
            self.budget.available_for_history(),
            self.compression_count + 1
        );

        // Phase 0: 预处理 — 裁剪旧工具输出（廉价，不需要 LLM）
        let messages = Self::prune_old_tool_results(messages);

        // Phase 1: 保护头部（system prompt）
        let (head, rest) = self.split_head(&messages);

        // Phase 2: 正向边界对齐 — 跳过头部之后的孤立 tool results
        let head_len = head.len();
        let aligned_start = Self::align_boundary_forward(rest, 0);
        let rest = &rest[aligned_start..];
        let head = &messages[..head_len + aligned_start];

        // Phase 3: 保护尾部（按 token 预算，soft_ceiling 防超大消息切割）
        let (middle, tail) = self.split_tail(rest);

        // Phase 3: 对中间部分生成摘要
        let mut summary_text = if let Some(text) = summary_text {
            // 使用 LLM 预计算的摘要
            let new_summary = StructuredSummary::from_text(text);
            if let Some(ref mut acc) = self.accumulated_summary {
                acc.merge(&new_summary);
                acc.to_text()
            } else {
                self.accumulated_summary = Some(new_summary.clone());
                new_summary.to_text()
            }
        } else {
            // 启发式摘要
            self.summarize_middle(middle)
        };
        session_memory.inject_into_summary(&mut summary_text);
        runtime_continuity.inject_into_summary(&mut summary_text);

        // Phase 4: 组装结果
        let mut result = head.to_vec();

        // 生成 Compact Boundary 元数据（在 summary 组装前准备）
        let compact_meta = if !summary_text.is_empty() {
            self.compact_sequence += 1;
            Some(CompactMetadata {
                sequence: self.compact_sequence,
                boundary_id: format!("cb-{}", Uuid::new_v4().simple()),
                preserved_tail_count: tail.len(),
                messages_before: original_message_count,
                messages_after: head.len() + tail.len() + 1, // +1 for summary
                tokens_before: original_tokens_before,
                tokens_after: 0, // 将在后面更新
                timestamp: chrono::Local::now().to_rfc3339(),
            })
        } else {
            None
        };

        if !summary_text.is_empty() {
            let mut formatted_summary = if self.compression_count > 0 {
                format!(
                    "{}\n（上下文已压缩 {} 次，保留累积知识）\n\n{}",
                    SUMMARY_PREFIX,
                    self.compression_count + 1,
                    summary_text
                )
            } else {
                format!("{}\n\n{}", SUMMARY_PREFIX, summary_text)
            };

            // 嵌入 Compact Boundary 标记
            if let Some(ref meta) = compact_meta {
                formatted_summary.push_str(&meta.to_boundary_marker());
            }

            // ── 消息角色交替（Hermes 风格）──
            // OpenAI API 要求消息角色交替，不能连续两个相同角色
            // 检查 head 最后一条和 tail 第一条的角色，选择合适的摘要角色
            let last_head_role = head
                .last()
                .map(|m| match m {
                    Message::System { .. } => "system",
                    Message::User { .. } => "user",
                    Message::Assistant { .. } => "assistant",
                    Message::Tool { .. } => "tool",
                })
                .unwrap_or("system");

            let first_tail_role = if tail.is_empty() {
                "none"
            } else {
                match &tail[0] {
                    Message::System { .. } => "system",
                    Message::User { .. } => "user",
                    Message::Assistant { .. } => "assistant",
                    Message::Tool { .. } => "tool",
                }
            };

            // 选择摘要消息的角色（优先避免与 head 碰撞）
            let summary_role = if last_head_role == "user" || last_head_role == "tool" {
                "assistant"
            } else {
                "user"
            };

            // 如果选择的角色与 tail 碰撞，且翻转不会与 head 碰撞，翻转
            let summary_role = if summary_role == first_tail_role {
                let flipped = if summary_role == "user" {
                    "assistant"
                } else {
                    "user"
                };
                if flipped != last_head_role {
                    flipped
                } else {
                    // 两个角色都会产生连续相同角色
                    // 将摘要合并到第一条 tail 消息中
                    "merge"
                }
            } else {
                summary_role
            };

            if summary_role == "merge" && !tail.is_empty() {
                // 合并模式：将摘要前置到第一条 tail 消息
                let mut merged_tail = tail.to_vec();
                let original_content = merged_tail[0].content();
                merged_tail[0] = match &merged_tail[0] {
                    Message::User { .. } => {
                        Message::user(format!("{}\n\n{}", formatted_summary, original_content))
                    }
                    Message::Assistant { content: _, .. } => {
                        // 需要保留 tool_calls
                        // 这里简化处理，直接用 user 消息
                        Message::user(format!("{}\n\n{}", formatted_summary, original_content))
                    }
                    _ => Message::user(format!("{}\n\n{}", formatted_summary, original_content)),
                };
                result.extend_from_slice(&merged_tail);
            } else {
                let summary_msg = match summary_role {
                    "assistant" => Message::assistant(&formatted_summary),
                    _ => Message::system(&formatted_summary),
                };
                result.push(summary_msg);
                result.extend_from_slice(tail);
            }
        } else {
            result.extend_from_slice(tail);
        }

        // Phase 5: 校验工具调用对完整性（移除孤立 tool result + 插入 stub）
        let result = Self::sanitize_tool_pairs(result);

        // 更新 compact metadata 的 tokens_after 并保存到历史
        let tokens_after = estimate_messages_tokens(&result);
        let mut recorded_meta = None;
        if let Some(mut meta) = compact_meta {
            meta.tokens_after = tokens_after;
            recorded_meta = Some(meta.clone());
            self.compact_metadata_history.push(meta);
        }

        self.total_tokens_after += tokens_after;
        let mut provenance = vec![
            format!(
                "level:{}",
                level.map(|value| value.label()).unwrap_or("summary")
            ),
            "summary:structured".to_string(),
            "tool_result_snip".to_string(),
            "tool_pair_sanitize".to_string(),
        ];
        if summary_text.contains("Frequently Accessed Files")
            || summary_text.contains("Pending Tasks")
            || summary_text.contains("Common Tool Patterns")
            || summary_text.contains("User Preferences")
        {
            provenance.push("summary_memory:session".to_string());
        }
        if !runtime_continuity.is_empty() {
            provenance.push("summary_memory:runtime_continuity".to_string());
        }
        provenance.push(if summary_text.is_empty() {
            "summary_source:empty".to_string()
        } else {
            summary_source_tag.to_string()
        });
        provenance.extend(session_memory.provenance_tags());
        provenance.extend(runtime_continuity.provenance_tags());
        self.record_compaction(CompactionRuntimeRecord {
            strategy,
            level: level.map(|value| value.label().to_string()),
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(original_tokens_before)),
            messages_before: original_message_count,
            messages_after: result.len(),
            tokens_before: original_tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(original_tokens_before, tokens_after),
            stage_order: compaction_stage_order(strategy),
            boundary_id: recorded_meta.as_ref().map(|meta| meta.boundary_id.clone()),
            sequence: recorded_meta.as_ref().map(|meta| meta.sequence),
            preserved_tail_count: recorded_meta.as_ref().map(|meta| meta.preserved_tail_count),
            retained_items: compaction_retained_items(
                head.len(),
                tail.len(),
                recorded_meta.as_ref(),
                &session_memory,
                &runtime_continuity,
            ),
            provenance,
        });
        self.compression_count += 1;

        info!(
            "Compressed to {} messages (compact_boundary #{})",
            result.len(),
            self.compact_sequence
        );
        result
    }

    /// 预处理：裁剪旧工具输出（替换为简短摘要）
    fn prune_old_tool_results(messages: &[Message]) -> Vec<Message> {
        let mut result = Vec::with_capacity(messages.len());
        // 保留最近 3 轮的工具输出，更早的裁剪
        let tool_msg_count = messages
            .iter()
            .filter(|m| matches!(m, Message::Tool { .. }))
            .count();
        let keep_last_n = 3;
        let mut tool_seen = 0;

        for msg in messages {
            match msg {
                Message::Tool {
                    tool_call_id,
                    content,
                } => {
                    tool_seen += 1;
                    let is_recent = tool_seen > tool_msg_count.saturating_sub(keep_last_n);
                    if is_recent || tool_msg_count <= keep_last_n {
                        result.push(msg.clone());
                    } else {
                        let keep_len = if Self::is_critical_tool_output(content) {
                            1000
                        } else {
                            200
                        };
                        // 裁剪：关键失败链路保留更多上下文，普通结果保留短摘要
                        let truncated = if content.len() > keep_len {
                            let safe: String = content.chars().take(keep_len).collect();
                            format!("{}...(truncated)", safe)
                        } else {
                            content.clone()
                        };
                        result.push(Message::Tool {
                            tool_call_id: tool_call_id.clone(),
                            content: truncated,
                        });
                    }
                }
                _ => result.push(msg.clone()),
            }
        }
        result
    }

    fn is_critical_tool_output(content: &str) -> bool {
        let lower = content.to_lowercase();
        let critical_markers = [
            "result: error",
            "error",
            "failed",
            "panic",
            "traceback",
            "diagnostic",
            "assertion",
            "permission denied",
            "cannot find",
            "no such file",
            "test failed",
        ];
        critical_markers.iter().any(|m| lower.contains(m))
    }

    /// 分离头部（system prompt）
    fn split_head<'a>(&self, messages: &'a [Message]) -> (&'a [Message], &'a [Message]) {
        let head_end = messages
            .iter()
            .position(|m| !matches!(m, Message::System { .. }))
            .unwrap_or(messages.len());
        (&messages[..head_end], &messages[head_end..])
    }

    /// 正向边界对齐：如果 compress_start 落在孤立的 tool result 上，
    /// 向前跳过它们，避免从 tool group 中间开始压缩区域。
    /// （Hermes: _align_boundary_forward）
    fn align_boundary_forward(messages: &[Message], idx: usize) -> usize {
        let mut i = idx;
        while i < messages.len() {
            if matches!(&messages[i], Message::Tool { .. }) {
                i += 1;
            } else {
                break;
            }
        }
        i
    }

    /// 分离尾部（按 token 预算 + soft_ceiling 保护）
    /// 包含 tool group boundary alignment（不切割 tool_call/tool_result 对）
    fn split_tail<'a>(&self, messages: &'a [Message]) -> (&'a [Message], &'a [Message]) {
        let target = self.budget.target_tokens();
        let soft_ceiling = self.budget.tail_soft_ceiling();
        let mut used_tokens = 0u64;
        let mut tail_start = messages.len();

        // 从后往前计算，使用 soft_ceiling 防止超大消息中间切割
        for (i, msg) in messages.iter().enumerate().rev() {
            let tokens = estimate_tokens(&msg.content()) + 4;
            if used_tokens + tokens > soft_ceiling {
                tail_start = i + 1;
                break;
            }
            used_tokens += tokens;
            // 如果在 target 内，继续；超过 target 但在 soft_ceiling 内，也继续
            if used_tokens > target && tail_start == messages.len() {
                // 记录第一个超过 target 的位置，作为备选
                tail_start = i + 1;
            }
        }

        // 确保至少保留一条消息
        if tail_start >= messages.len() && !messages.is_empty() {
            tail_start = messages.len() - 1;
        }

        // ── Tool group boundary alignment ──────────────
        // 如果 tail_start 落在 tool result 中，需要把对应的 assistant 消息也包含进来
        // 如果 tail_start 落在 assistant + tool_calls 中，需要把所有 tool result 也包含进来
        if tail_start < messages.len() {
            // 检查 tail_start 是否在 tool result 链中间
            if let Message::Tool { tool_call_id, .. } = &messages[tail_start] {
                // 找到发起这个 tool_call 的 assistant 消息
                let call_id = tool_call_id.clone();
                for j in (0..tail_start).rev() {
                    if let Message::Assistant {
                        tool_calls: Some(calls),
                        ..
                    } = &messages[j]
                    {
                        if calls.iter().any(|tc| tc.id == call_id) {
                            // 将 tail_start 扩展到 assistant 消息
                            tail_start = j;
                            break;
                        }
                    }
                }
            }

            // 检查 tail_start 是否是带 tool_calls 的 assistant
            if let Message::Assistant {
                tool_calls: Some(calls),
                ..
            } = &messages[tail_start]
            {
                if !calls.is_empty() {
                    // 找到最后一个 tool result 的位置
                    let call_ids: std::collections::HashSet<&str> =
                        calls.iter().map(|tc| tc.id.as_str()).collect();
                    let mut last_result_idx = tail_start;
                    #[allow(clippy::needless_range_loop)]
                    for j in (tail_start + 1)..messages.len() {
                        if let Message::Tool { tool_call_id, .. } = &messages[j] {
                            if call_ids.contains(tool_call_id.as_str()) {
                                last_result_idx = j;
                            }
                        } else {
                            break; // tool results 必须连续
                        }
                    }
                    // 确保所有 tool results 都在 tail 中
                    // （tail 已经包含 tail_start 之后的所有消息，所以这里不需要调整）
                    let _ = last_result_idx;
                }
            }
        }

        // 最少保留 3 条消息（Hermes 风格）
        if tail_start >= messages.len().saturating_sub(2) && messages.len() > 3 {
            tail_start = messages.len() - 3;
        }

        (&messages[..tail_start], &messages[tail_start..])
    }

    /// 对中间部分生成摘要（迭代式）
    fn summarize_middle(&mut self, messages: &[Message]) -> String {
        if messages.is_empty() {
            return self
                .accumulated_summary
                .as_ref()
                .map(|s| s.to_text())
                .unwrap_or_default();
        }

        // 启发式提取
        let mut new_summary = StructuredSummary::new();
        new_summary.goal = format!("对话包含 {} 条消息", messages.len());

        let mut tool_calls = Vec::new();
        let mut files = Vec::new();
        let mut user_goals = Vec::new();

        for msg in messages {
            match msg {
                Message::User { content } => {
                    // 提取用户目标（第一条用户消息通常是目标描述）
                    if user_goals.is_empty() && content.len() > 10 {
                        user_goals.push(content.chars().take(200).collect::<String>());
                    }
                }
                Message::Assistant {
                    tool_calls: Some(calls),
                    ..
                } => {
                    for tc in calls {
                        if !tool_calls.contains(&tc.name) {
                            tool_calls.push(tc.name.clone());
                        }
                        // 提取文件路径
                        if let Some(path) = tc.arguments.get("path").and_then(|v| v.as_str()) {
                            if !files.contains(&path.to_string()) {
                                files.push(path.to_string());
                            }
                        }
                        // 保留关键命令参数（尤其是编译/测试/诊断命令）
                        if tc.name == "bash" {
                            if let Some(cmd) = tc.arguments["command"]
                                .as_str()
                                .or_else(|| tc.arguments["cmd"].as_str())
                            {
                                let trimmed = cmd.trim();
                                if !trimmed.is_empty() {
                                    let snippet = format!(
                                        "Command: {}",
                                        trimmed.chars().take(180).collect::<String>()
                                    );
                                    if !new_summary.tools_patterns.contains(&snippet) {
                                        new_summary.tools_patterns.push(snippet);
                                    }
                                }
                            }
                        }
                    }
                }
                Message::Tool { content, .. } => {
                    // 只有当工具结果同时包含错误和成功标志时，才认为"错误已解决"
                    let lower = content.to_lowercase();
                    let has_error = lower.contains("error") || lower.contains("failed");
                    let has_success = lower.contains("ok")
                        || lower.contains("success")
                        || lower.contains("passed");
                    if has_error && has_success {
                        new_summary
                            .progress_done
                            .push("遇到错误并已解决".to_string());
                    }
                    // 启发式提取：保留关键工具输出（文件内容、诊断结果等）
                    let trimmed = content.trim();
                    if !trimmed.is_empty()
                        && trimmed.len() > 20
                        && trimmed.len() < 300
                        && (trimmed.contains("API key")
                            || trimmed.contains("secret")
                            || trimmed.contains("password")
                            || trimmed.contains("diagnostic"))
                    {
                        let snippet = trimmed.chars().take(200).collect::<String>();
                        if !new_summary.critical_context.contains(&snippet) {
                            new_summary.critical_context.push(snippet);
                        }
                    }
                    // 长输出中提取失败链路和关键诊断行
                    let lower = content.to_lowercase();
                    if Self::is_critical_tool_output(content)
                        || lower.contains("cargo check")
                        || lower.contains("cargo test")
                    {
                        let important_lines: Vec<String> = content
                            .lines()
                            .map(str::trim)
                            .filter(|l| {
                                let x = l.to_lowercase();
                                !l.is_empty()
                                    && (x.contains("error")
                                        || x.contains("failed")
                                        || x.contains("panic")
                                        || x.contains("warning")
                                        || x.contains("cargo check")
                                        || x.contains("cargo test")
                                        || x.contains("diagnostic")
                                        || x.contains("line "))
                            })
                            .take(6)
                            .map(|s| s.chars().take(180).collect::<String>())
                            .collect();
                        for line in important_lines {
                            if !new_summary.critical_context.contains(&line) {
                                new_summary.critical_context.push(line);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !user_goals.is_empty() {
            new_summary.goal = user_goals[0].clone();
        }

        if !tool_calls.is_empty() {
            for tool in tool_calls {
                if !new_summary.tools_patterns.contains(&tool) {
                    new_summary.tools_patterns.push(tool);
                }
            }
        }
        if !files.is_empty() {
            new_summary.files_modified = files;
        }

        // 迭代式合并：将新摘要合并到累积摘要
        if let Some(ref mut acc) = self.accumulated_summary {
            acc.merge(&new_summary);
            acc.to_text()
        } else {
            self.accumulated_summary = Some(new_summary.clone());
            new_summary.to_text()
        }
    }

    /// 使用 LLM 生成高质量结构化摘要（异步）
    /// 需要先通过 with_llm_provider() 设置 provider
    pub async fn llm_summarize_middle(&self, messages: &[Message]) -> Option<String> {
        let provider = self.llm_provider.as_ref()?;
        if messages.is_empty() {
            return None;
        }

        // 构建对话文本
        let mut conversation = String::new();
        for msg in messages {
            let (role, content) = match msg {
                Message::User { content } => ("User", content.as_str()),
                Message::Assistant { content, .. } => ("Assistant", content.as_str()),
                Message::Tool { content, .. } => ("Tool Result", content.as_str()),
                Message::System { content } => ("System", content.as_str()),
            };
            conversation.push_str(&format!("{}: {}\n\n", role, content));
        }

        let prompt = format!(
            "Summarize this conversation into 8 sections: Goal, Constraints & Preferences, Progress (Done/InProgress/Blocked), Key Decisions, Relevant Files, Next Steps, Critical Context, Tools & Patterns.\n\n{}",
            &conversation.chars().take(8000).collect::<String>()
        );

        let request = crate::services::api::ChatRequest::new(&self.llm_model)
            .with_messages(vec![crate::services::api::Message::user(&prompt)]);

        match provider.chat(request).await {
            Ok(response) => {
                debug!("LLM summary generated ({} chars)", response.content.len());
                Some(response.content)
            }
            Err(e) => {
                warn!("LLM summarization failed: {}, falling back to heuristic", e);
                None
            }
        }
    }

    /// 检查是否有 LLM provider 可用
    pub fn has_llm_provider(&self) -> bool {
        self.llm_provider.is_some()
    }

    /// 记录压缩失败（进入冷却期）
    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(std::time::Instant::now());
        debug!(
            "Compression failed, entering cooldown for {}s",
            self.cooldown_secs
        );
    }

    /// 校验工具调用对的完整性
    /// 确保每个 tool_call 都有对应的 tool result，反之亦然
    fn sanitize_tool_pairs(mut messages: Vec<Message>) -> Vec<Message> {
        let mut pending_tool_calls: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut to_remove = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            match msg {
                Message::Assistant {
                    tool_calls: Some(calls),
                    ..
                } => {
                    for tc in calls {
                        pending_tool_calls.insert(tc.id.clone(), i);
                    }
                }
                Message::Tool { tool_call_id, .. } => {
                    if pending_tool_calls.remove(tool_call_id).is_none() {
                        // 没有对应的 tool_call，标记移除
                        to_remove.push(i);
                    }
                }
                _ => {}
            }
        }

        // 移除孤立的 tool result
        for i in to_remove.into_iter().rev() {
            messages.remove(i);
        }

        // 为没有 result 的 tool_call 插入 stub
        if !pending_tool_calls.is_empty() {
            debug!(
                "Found {} orphaned tool calls, inserting stubs",
                pending_tool_calls.len()
            );
            for (tc_id, assistant_idx) in &pending_tool_calls {
                // 在 assistant 消息之后插入 stub tool result
                let insert_pos = assistant_idx + 1;
                if insert_pos <= messages.len() {
                    messages.insert(
                        insert_pos,
                        Message::Tool {
                            tool_call_id: tc_id.clone(),
                            content: "[Tool result lost during context compression]".to_string(),
                        },
                    );
                }
            }
        }

        messages
    }

    /// 获取当前累积摘要的引用
    pub fn accumulated_summary(&self) -> Option<&StructuredSummary> {
        self.accumulated_summary.as_ref()
    }

    /// 获取压缩元数据历史
    pub fn compact_metadata_history(&self) -> &[CompactMetadata] {
        &self.compact_metadata_history
    }

    /// 获取最近一次 compact boundary 元数据
    pub fn latest_compact_metadata(&self) -> Option<&CompactMetadata> {
        self.compact_metadata_history.last()
    }

    fn record_compaction(&mut self, mut record: CompactionRuntimeRecord) {
        record.normalize_provenance();
        self.compaction_records.push(record);
    }

    /// 获取运行时压缩记录（策略、来源和 compact boundary）。
    pub fn compaction_records(&self) -> &[CompactionRuntimeRecord] {
        &self.compaction_records
    }

    pub fn compaction_attempt_records(&self) -> &[CompactionAttemptRecord] {
        &self.compaction_attempt_records
    }

    pub fn compaction_circuit_open(&self) -> bool {
        self.consecutive_compaction_failures >= self.max_consecutive_compaction_failures
            || self.consecutive_no_gain_compactions >= self.max_consecutive_no_gain_compactions
    }

    pub fn record_compaction_decision(
        &mut self,
        input: CompactionAttemptInput,
    ) -> CompactionAttemptRecord {
        match input.decision {
            CompactionDecision::Compacted | CompactionDecision::Recovered => {
                self.consecutive_compaction_failures = 0;
                self.consecutive_no_gain_compactions = 0;
            }
            CompactionDecision::NoGain => {
                self.consecutive_no_gain_compactions =
                    self.consecutive_no_gain_compactions.saturating_add(1);
            }
            CompactionDecision::Failed => {
                self.consecutive_compaction_failures =
                    self.consecutive_compaction_failures.saturating_add(1);
            }
            _ => {}
        }
        let record = CompactionAttemptRecord {
            trigger: input.trigger,
            strategy: input.strategy,
            decision: input.decision,
            before_tokens: input.before_tokens,
            after_tokens: input.after_tokens,
            messages_before: input.messages_before,
            messages_after: input.messages_after,
            reason: input.reason,
            attempt_index: self
                .compaction_attempt_records
                .len()
                .saturating_add(1)
                .try_into()
                .unwrap_or(u32::MAX),
            consecutive_no_gain: self.consecutive_no_gain_compactions,
            consecutive_failures: self.consecutive_compaction_failures,
            circuit_open: self.compaction_circuit_open(),
            boundary_id: input.boundary_id,
        };
        self.compaction_attempt_records.push(record.clone());
        record
    }

    pub fn annotate_compaction_record_trigger(&mut self, index: usize, trigger: impl Into<String>) {
        if let Some(record) = self.compaction_records.get_mut(index) {
            record.trigger = Some(trigger.into());
            record.normalize_provenance();
        }
    }

    /// 获取最近一次运行时压缩记录。
    pub fn latest_compaction_record(&self) -> Option<&CompactionRuntimeRecord> {
        self.compaction_records.last()
    }

    /// 获取压缩统计
    pub fn stats(&self) -> CompressionStats {
        let savings_rate = if self.total_tokens_before > 0 {
            self.total_tokens_before
                .saturating_sub(self.total_tokens_after)
                .saturating_mul(100)
                / self.total_tokens_before
        } else {
            0
        };
        CompressionStats {
            compression_count: self.compression_count,
            max_context_tokens: self.budget.max_context_tokens,
            available_tokens: self.budget.available_for_history(),
            has_accumulated_summary: self.accumulated_summary.is_some(),
            in_cooldown: self.is_in_cooldown(),
            total_tokens_before: self.total_tokens_before,
            total_tokens_after: self.total_tokens_after,
            llm_compression_attempts: self.llm_compression_attempts,
            llm_compression_failures: self.llm_compression_failures,
            savings_rate,
            session_duration_secs: self.session_start.elapsed().as_secs(),
            message_count: 0, // caller should fill this
            time_based_enabled: self.time_config.enabled,
        }
    }
}

/// 压缩统计
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub compression_count: u32,
    pub max_context_tokens: u64,
    pub available_tokens: u64,
    pub has_accumulated_summary: bool,
    pub in_cooldown: bool,
    /// 累积压缩前 token 数
    pub total_tokens_before: u64,
    /// 累积压缩后 token 数
    pub total_tokens_after: u64,
    /// LLM 压缩尝试次数
    pub llm_compression_attempts: u32,
    /// LLM 压缩失败次数
    pub llm_compression_failures: u32,
    /// 累积节省率（百分比）
    pub savings_rate: u64,
    /// 会话时长（秒）
    pub session_duration_secs: u64,
    /// 当前消息数
    pub message_count: usize,
    /// 时间基础压缩是否启用
    pub time_based_enabled: bool,
}

// ── 测试 ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 防止编译器过度优化的简单 black_box
    fn black_box<T>(x: T) -> T {
        std::hint::black_box(x)
    }

    #[test]
    fn test_token_budget() {
        let budget = TokenBudget::new(128_000);
        assert_eq!(budget.available_for_history(), 128_000 - 4096 - 2000 - 1000);
        assert!(budget.needs_compression(100_000));
        assert!(!budget.needs_compression(50_000));
    }

    #[test]
    fn test_tail_soft_ceiling() {
        let budget = TokenBudget::new(128_000);
        let target = budget.target_tokens();
        let ceiling = budget.tail_soft_ceiling();
        assert!(ceiling > target);
        assert_eq!(ceiling, target * 150 / 100);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 2); // 5 chars / 4 = 1.25 → 2
        assert_eq!(estimate_tokens("1234"), 1); // 4 chars / 4 = 1
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_structured_summary_8_sections() {
        let mut s = StructuredSummary::new();
        s.goal = "Build auth".to_string();
        s.constraints.push("Must use JWT".to_string());
        s.progress_done.push("Login done".to_string());
        s.decisions.push("Use bcrypt".to_string());
        s.files_modified.push("auth.rs".to_string());
        s.next_steps.push("Add OAuth".to_string());
        s.critical_context.push("API key in .env".to_string());
        s.tools_patterns.push("grep before edit".to_string());

        let text = s.to_text();

        assert!(text.contains("## Goal"));
        assert!(text.contains("## Constraints"));
        assert!(text.contains("## Progress"));
        assert!(text.contains("## Key Decisions"));
        assert!(text.contains("## Relevant Files"));
        assert!(text.contains("## Next Steps"));
        assert!(text.contains("## Critical Context"));
        assert!(text.contains("## Tools & Patterns"));
    }

    #[test]
    fn test_structured_summary_merge() {
        let mut s1 = StructuredSummary::new();
        s1.goal = "Build auth".to_string();
        s1.progress_done.push("Login done".to_string());
        s1.files_modified.push("auth.rs".to_string());
        s1.critical_context.push("JWT secret in env".to_string());

        let mut s2 = StructuredSummary::new();
        s2.goal = "Build auth v2".to_string();
        s2.progress_done.push("Signup done".to_string());
        s2.next_steps.push("Add OAuth".to_string());
        s2.critical_context.push("Rate limit: 100/min".to_string());

        s1.merge(&s2);

        assert_eq!(s1.goal, "Build auth v2"); // goal 被更新
        assert_eq!(s1.progress_all().len(), 2); // progress 累积
        assert_eq!(s1.files_modified.len(), 1); // files 保留
        assert_eq!(s1.next_steps.len(), 1); // next_steps 被更新
        assert_eq!(s1.critical_context.len(), 2); // critical_context 累积
    }

    #[test]
    fn test_summary_from_text() {
        let text = r#"## Goal
实现用户认证

## Constraints
- 必须使用 JWT
- 密码用 bcrypt

## Progress
- 完成了登录 API
- 添加了 JWT 支持

## Key Decisions
- 选择 bcrypt 而非 argon2

## Relevant Files
- src/auth.rs

## Next Steps
- 添加 OAuth

## Critical Context
- API key 存放在 .env 文件中

## Tools & Patterns
- 先 grep 再 edit"#;

        let summary = StructuredSummary::from_text(text);
        assert_eq!(summary.goal, "实现用户认证");
        assert_eq!(summary.constraints.len(), 2);
        assert_eq!(summary.progress_all().len(), 2);
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.files_modified.len(), 1);
        assert_eq!(summary.next_steps.len(), 1);
        assert_eq!(summary.critical_context.len(), 1);
        assert_eq!(summary.tools_patterns.len(), 1);
    }

    #[test]
    fn test_compress_preserves_head_and_tail() {
        let mut compressor = ContextCompressor::new(1000);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine, thanks!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress(&messages);

        // 头部 system prompt 应该保留
        assert!(matches!(&compressed[0], Message::System { .. }));

        // 应该有摘要或尾部消息
        assert!(compressed.len() >= 2);

        // 统计
        let stats = compressor.stats();
        assert_eq!(stats.compression_count, 1);
    }

    #[test]
    fn test_sanitize_tool_pairs_removes_orphans() {
        let messages = vec![
            Message::user("Run ls"),
            Message::assistant_with_tools(
                "Running...",
                vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            ),
            Message::tool("call_1", "file1.txt\nfile2.txt"),
            // 孤立的 tool result（没有对应的 call）
            Message::tool("call_orphan", "some result"),
        ];

        let sanitized = ContextCompressor::sanitize_tool_pairs(messages);
        // 孤立的 tool result 应该被移除
        assert_eq!(sanitized.len(), 3);
    }

    #[test]
    fn test_sanitize_tool_pairs_inserts_stubs() {
        let messages = vec![
            Message::user("Run ls"),
            Message::assistant_with_tools(
                "Running...",
                vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            ),
            // 没有 tool result — 应该插入 stub
            Message::user("Next question"),
        ];

        let sanitized = ContextCompressor::sanitize_tool_pairs(messages);
        // 应该有 4 条消息（插入了 stub）
        assert_eq!(sanitized.len(), 4);
        // stub 应该是 tool result
        if let Message::Tool {
            tool_call_id,
            content,
        } = &sanitized[2]
        {
            assert_eq!(tool_call_id, "call_1");
            assert!(content.contains("lost"));
        } else {
            panic!("Expected stub tool result at index 2");
        }
    }

    #[test]
    fn test_cooldown() {
        let mut compressor = ContextCompressor::new(1000);
        assert!(!compressor.is_in_cooldown());

        compressor.record_failure();
        assert!(compressor.is_in_cooldown());
    }

    #[test]
    fn test_preflight_check() {
        let compressor = ContextCompressor::new(10_000);
        let messages = vec![Message::user("x".repeat(5000))];

        // 不超阈值
        assert!(!compressor.preflight_check(&messages, 100, 100));

        // 超阈值
        assert!(compressor.preflight_check(&messages, 5000, 5000));
    }

    #[test]
    fn compaction_attempt_records_open_circuit_after_repeated_no_gain() {
        let mut compressor = ContextCompressor::new(10_000);

        let first = compressor.record_compaction_decision(
            CompactionAttemptInput::new(
                "test",
                ContextCompactionStrategy::AutoCompact,
                CompactionDecision::NoGain,
                1_000,
                4,
                "no reduction",
            )
            .with_after(Some(1_000), Some(4)),
        );
        assert!(!first.circuit_open);

        let second = compressor.record_compaction_decision(
            CompactionAttemptInput::new(
                "test",
                ContextCompactionStrategy::AutoCompact,
                CompactionDecision::NoGain,
                1_000,
                4,
                "no reduction",
            )
            .with_after(Some(1_000), Some(4)),
        );
        assert!(second.circuit_open);
        assert!(compressor.compaction_circuit_open());
        assert_eq!(compressor.compaction_attempt_records().len(), 2);
        assert_eq!(
            compressor.compaction_attempt_records()[1].decision,
            CompactionDecision::NoGain
        );
    }

    #[test]
    fn successful_compaction_attempt_resets_circuit_counters() {
        let mut compressor = ContextCompressor::new(10_000);
        compressor.record_compaction_decision(
            CompactionAttemptInput::new(
                "test",
                ContextCompactionStrategy::AutoCompact,
                CompactionDecision::NoGain,
                1_000,
                4,
                "no reduction",
            )
            .with_after(Some(1_000), Some(4)),
        );
        let compacted = compressor.record_compaction_decision(
            CompactionAttemptInput::new(
                "test",
                ContextCompactionStrategy::AutoCompact,
                CompactionDecision::Compacted,
                1_000,
                4,
                "reduced",
            )
            .with_after(Some(500), Some(2))
            .with_boundary_id(Some("boundary-1".to_string())),
        );
        assert_eq!(compacted.consecutive_no_gain, 0);
        assert_eq!(compacted.consecutive_failures, 0);
        assert!(!compacted.circuit_open);
    }

    #[test]
    fn test_align_boundary_forward_skips_orphan_tools() {
        // 头部之后有孤立的 tool results（被 summarize 后残留）
        let messages = vec![
            Message::system("You are helpful"),
            Message::tool("call_orphan_1", "old result 1"),
            Message::tool("call_orphan_2", "old result 2"),
            Message::user("What's next?"),
            Message::assistant("Let me check"),
        ];

        // align_boundary_forward 应该跳过孤立 tool results
        let aligned = ContextCompressor::align_boundary_forward(&messages, 1);
        assert_eq!(aligned, 3); // 跳过 index 1, 2（两个 tool messages）
    }

    #[test]
    fn test_align_boundary_forward_no_tools() {
        // 没有孤立 tool results，idx 不变
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let aligned = ContextCompressor::align_boundary_forward(&messages, 0);
        assert_eq!(aligned, 0); // 第一条就是 user，不变
    }

    #[test]
    fn test_summary_prefix_in_output() {
        let mut compressor = ContextCompressor::new(1000);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress(&messages);

        // 找到摘要消息，应该包含 SUMMARY_PREFIX
        let has_prefix = compressed.iter().any(|m| {
            let content = m.content();
            content.contains("[CONTEXT COMPACTION]")
        });
        assert!(
            has_prefix,
            "Compressed output should contain SUMMARY_PREFIX"
        );
    }

    #[test]
    fn test_prune_keeps_more_context_for_critical_tool_output() {
        let mut messages = vec![Message::user("start"), Message::assistant("ok")];
        for i in 0..6 {
            let content = if i == 0 {
                format!("Result: ERROR\n{}\n", "x".repeat(1500))
            } else {
                "Result: OK\nsmall output".to_string()
            };
            messages.push(Message::tool(format!("call_{}", i), content));
        }

        let pruned = ContextCompressor::prune_old_tool_results(&messages);
        let first_tool = pruned
            .iter()
            .find_map(|m| match m {
                Message::Tool {
                    tool_call_id,
                    content,
                } if tool_call_id == "call_0" => Some(content.clone()),
                _ => None,
            })
            .expect("missing call_0");

        assert!(
            first_tool.len() > 200,
            "critical tool output should preserve more context"
        );
    }

    #[test]
    fn test_summarize_middle_extracts_command_and_error_lines() {
        let mut compressor = ContextCompressor::new(1000);
        let middle = vec![
            Message::assistant_with_tools(
                "run checks",
                vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "cargo check && cargo test"}),
                }],
            ),
            Message::tool(
                "call_1",
                "cargo check\nerror[E0425]: cannot find value `x` in this scope\nfailed to compile",
            ),
        ];

        let summary = compressor.summarize_middle(&middle);
        assert!(summary.contains("Command: cargo check && cargo test"));
        assert!(summary.to_lowercase().contains("error"));
    }

    #[test]
    fn test_role_alternation_no_consecutive_same() {
        let mut compressor = ContextCompressor::new(1000);

        // 构造一个会触发压缩的消息序列
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress(&messages);

        // 检查没有连续相同角色（除了 system 开头 + tool 消息）
        for i in 1..compressed.len() {
            let prev_role = match &compressed[i - 1] {
                Message::User { .. } => "user",
                Message::Assistant { .. } => "assistant",
                Message::System { .. } => "system",
                Message::Tool { .. } => "tool",
            };
            let curr_role = match &compressed[i] {
                Message::User { .. } => "user",
                Message::Assistant { .. } => "assistant",
                Message::System { .. } => "system",
                Message::Tool { .. } => "tool",
            };
            // 不允许 user-user 或 assistant-assistant 连续
            if prev_role == "user" || prev_role == "assistant" {
                assert_ne!(
                    prev_role,
                    curr_role,
                    "Found consecutive {} messages at index {}-{}",
                    prev_role,
                    i - 1,
                    i
                );
            }
        }
    }

    // ── Micro-benchmarks ──

    #[test]
    fn bench_compress_heuristic() {
        let mut messages = vec![Message::system("You are a helpful assistant.")];
        for i in 0..100 {
            messages.push(Message::user(format!("User message number {}", i)));
            messages.push(Message::assistant(format!("Assistant reply number {}", i)));
        }
        // 添加一些 tool 消息对
        for i in 0..20 {
            messages.push(Message::user(format!("Run command {}", i)));
            messages.push(Message::assistant_with_tools(
                "Running...",
                vec![ToolCall {
                    id: format!("call_{}", i),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            ));
            messages.push(Message::tool(format!("call_{}", i), "file.txt\n"));
        }

        let iterations = 500;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let mut compressor = ContextCompressor::new(2000);
            let result = compressor.compress(&messages);
            let _ = black_box(result);
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!(
            "bench_compress_heuristic: {} iterations, avg {:.1} μs/iter",
            iterations, avg_us
        );
    }

    #[test]
    fn bench_sanitize_tool_pairs() {
        let mut messages = vec![Message::user("Start")];
        for i in 0..50 {
            messages.push(Message::assistant_with_tools(
                "Running...",
                vec![ToolCall {
                    id: format!("call_{}", i),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            ));
            messages.push(Message::tool(format!("call_{}", i), "result"));
        }
        // 添加孤立的 tool result
        messages.push(Message::tool("orphan", "orphan result"));

        let iterations = 5000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let result = ContextCompressor::sanitize_tool_pairs(messages.clone());
            let _ = black_box(result);
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!(
            "bench_sanitize_tool_pairs: {} iterations, avg {:.1} μs/iter",
            iterations, avg_us
        );
    }

    #[test]
    fn bench_estimate_messages_tokens() {
        let mut messages = vec![Message::system("You are a helpful assistant.")];
        for i in 0..200 {
            messages.push(Message::user(format!("User message number {}", i)));
            messages.push(Message::assistant(format!("Assistant reply number {}", i)));
        }

        let iterations = 10_000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let tokens = estimate_messages_tokens(&messages);
            let _ = black_box(tokens);
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
        println!(
            "bench_estimate_messages_tokens: {} iterations, avg {:.0} ns/iter",
            iterations, avg_ns
        );
    }

    // ── LLM 压缩测试 ───────────────────────────────────────────────────────────────────

    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Usage};
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;

    struct MockLlmProvider {
        response: Option<String>,
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            match self.response.as_ref() {
                Some(content) => Ok(ChatResponse {
                    content: content.clone(),
                    tool_calls: None,
                    usage: Some(Usage {
                        prompt_tokens: 100,
                        completion_tokens: 50,
                        total_tokens: 150,
                        reasoning_tokens: None,
                        cached_tokens: None,
                    }),
                }),
                None => Err(anyhow::anyhow!("Mock LLM error")),
            }
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            unimplemented!()
        }

        fn base_url(&self) -> &str {
            "http://localhost"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn test_compress_async_with_llm_success() {
        let summary_text = "## Goal\nTest goal\n\n## Constraints\n\n## Progress\n\n## Key Decisions\n\n## Relevant Files\n\n## Next Steps\n\n## Critical Context\n\n## Tools & Patterns\n";
        let provider = std::sync::Arc::new(MockLlmProvider {
            response: Some(summary_text.to_string()),
        });

        let mut compressor = ContextCompressor::new(1000).with_llm_provider(provider, "mock-model");

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine, thanks!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress_async(&messages).await;

        // 应该生成摘要消息
        let has_summary = compressed.iter().any(|m| {
            let content = m.content();
            content.contains("[CONTEXT COMPACTION]")
        });
        assert!(
            has_summary,
            "LLM compression should produce a summary message"
        );

        let stats = compressor.stats();
        assert_eq!(stats.compression_count, 1);
        assert_eq!(stats.llm_compression_attempts, 1);
        assert_eq!(stats.llm_compression_failures, 0);
        assert!(stats.total_tokens_before > 0);
        assert!(stats.total_tokens_after > 0);
    }

    #[tokio::test]
    async fn test_compress_async_falls_back_when_llm_fails() {
        let provider = std::sync::Arc::new(MockLlmProvider { response: None });

        let mut compressor = ContextCompressor::new(1000).with_llm_provider(provider, "mock-model");

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine, thanks!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress_async(&messages).await;

        // 即使 LLM 失败，也应该有压缩输出（启发式）
        assert!(!compressed.is_empty());

        let stats = compressor.stats();
        assert_eq!(stats.llm_compression_attempts, 1);
        assert_eq!(stats.llm_compression_failures, 1);
        assert!(stats.in_cooldown);
    }

    #[tokio::test]
    async fn test_compress_async_without_provider_uses_heuristic() {
        let mut compressor = ContextCompressor::new(1000);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("I'm fine, thanks!"),
            Message::user("What's 2+2?"),
            Message::assistant("4"),
        ];

        let compressed = compressor.compress_async(&messages).await;

        assert!(!compressed.is_empty());

        let stats = compressor.stats();
        assert_eq!(stats.llm_compression_attempts, 0);
        assert_eq!(stats.llm_compression_failures, 0);
    }

    // ─── Long Session Stress Tests ─────────────────────────────────────────────

    /// Helper to create a long conversation with many turns
    fn create_long_conversation(turns: usize) -> Vec<Message> {
        let mut messages = vec![Message::system("You are a helpful coding assistant.")];
        for i in 0..turns {
            messages.push(Message::user(format!("Task {}: Implement feature X", i)));
            messages.push(Message::assistant(format!(
                "I'll implement feature X for task {}. Here's my approach...",
                i
            )));
            // Add some tool calls
            messages.push(Message::assistant_with_tools(
                format!("Tool use for task {}", i),
                vec![crate::services::api::ToolCall {
                    id: format!("call_{}", i),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "echo done"}),
                }],
            ));
            messages.push(Message::tool(
                format!("call_{}", i),
                "Command executed successfully".to_string(),
            ));
        }
        messages
    }

    #[test]
    fn test_long_session_50_turns_stability() {
        // 50 turns stress test - should remain stable
        let messages = create_long_conversation(50);
        let tokens = estimate_messages_tokens(&messages);

        // With 50 turns, we should have token usage (threshold adjusted for estimation method)
        assert!(
            tokens > 1000,
            "50 turns should use >1000 tokens, got {}",
            tokens
        );

        // Test that micro_compress doesn't panic and produces valid output
        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.micro_compress(&messages);

        // Compressed messages should still be valid
        assert!(!compressed.is_empty());

        // Check stats show micro_compress ran
        let stats = compressor.stats();
        assert!(stats.total_tokens_before > 0, "Should track tokens before");
    }

    #[test]
    fn test_long_session_100_turns_stability() {
        // 100 turns stress test - compression should trigger
        let messages = create_long_conversation(100);
        let tokens = estimate_messages_tokens(&messages);

        // With 100 turns, significant token usage
        assert!(
            tokens > 2000,
            "100 turns should use >2000 tokens, got {}",
            tokens
        );

        // Test micro_compress
        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.micro_compress(&messages);

        assert!(!compressed.is_empty());
        // micro_compress trims tool results but doesn't remove messages

        let stats = compressor.stats();
        assert!(stats.total_tokens_before > 0);
    }

    #[test]
    fn test_long_session_200_turns_stability() {
        // 200 turns stress test - aggressive compression
        let messages = create_long_conversation(200);
        let tokens = estimate_messages_tokens(&messages);

        // With 200 turns, very high token usage
        assert!(
            tokens > 4000,
            "200 turns should use >4000 tokens, got {}",
            tokens
        );

        // Test micro_compress handles large inputs
        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.micro_compress(&messages);

        assert!(!compressed.is_empty());

        // Multiple micro_compress calls should be stable
        let recompressed = compressor.micro_compress(&compressed);
        assert!(!recompressed.is_empty());
    }

    #[test]
    fn test_micro_compress_quality_preservation() {
        // Verify that micro_compress preserves critical content
        let mut messages = vec![Message::system("You are a helpful assistant.")];
        messages.push(Message::user(
            "Remember: the API endpoint is at localhost:8080".to_string(),
        ));
        messages.push(Message::assistant(
            "I'll remember that the API is at localhost:8080".to_string(),
        ));

        // Add many filler messages
        for i in 0..50 {
            messages.push(Message::user(format!("Turn {}", i)));
            messages.push(Message::assistant(format!("Response {}", i)));
        }

        // Critical info should be preserved - check in original messages
        let api_reference = "localhost:8080";
        let has_critical = messages.iter().any(|m| match m {
            Message::User { content, .. } | Message::Assistant { content, .. } => {
                content.contains(api_reference)
            }
            _ => false,
        });
        assert!(
            has_critical,
            "Original messages should contain critical info"
        );

        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.micro_compress(&messages);

        // After compression, the critical info should still be present
        // (micro_compress doesn't remove content, just trims tool results)
        let preserved = compressed.iter().any(|m| match m {
            Message::User { content, .. } | Message::Assistant { content, .. } => {
                content.contains(api_reference)
            }
            _ => false,
        });
        assert!(
            preserved,
            "Compressed messages should preserve critical info"
        );
    }

    #[test]
    fn test_compress_50_turns_stability() {
        // 50 turns: full compression pipeline should remain stable
        let messages = create_long_conversation(50);
        let tokens_before = estimate_messages_tokens(&messages);

        let mut compressor = ContextCompressor::new(32_000);
        let compressed = compressor.compress(&messages);
        let tokens_after = estimate_messages_tokens(&compressed);

        assert!(!compressed.is_empty());
        assert!(
            tokens_after < tokens_before,
            "Compression should reduce tokens: {} -> {}",
            tokens_before,
            tokens_after
        );

        let stats = compressor.stats();
        assert!(
            stats.compression_count >= 1,
            "Should have compressed at least once"
        );
    }

    #[test]
    fn test_compress_100_turns_stability() {
        // 100 turns: aggressive compression
        let messages = create_long_conversation(100);
        let tokens_before = estimate_messages_tokens(&messages);

        let mut compressor = ContextCompressor::new(32_000);
        let compressed = compressor.compress(&messages);
        let tokens_after = estimate_messages_tokens(&compressed);

        assert!(!compressed.is_empty());
        assert!(
            tokens_after < tokens_before,
            "Compression should reduce tokens: {} -> {}",
            tokens_before,
            tokens_after
        );

        // Multiple compressions should be stable
        let recompressed = compressor.compress(&compressed);
        assert!(!recompressed.is_empty());

        let stats = compressor.stats();
        assert!(stats.compression_count >= 2);
    }

    #[test]
    fn test_compression_level_auto_select() {
        // Low usage -> Light
        let level = CompressionLevel::auto_select(0.5, 0, 0, true);
        assert_eq!(level, CompressionLevel::Light);

        // Medium usage with LLM -> Medium
        let level = CompressionLevel::auto_select(0.75, 0, 0, true);
        assert_eq!(level, CompressionLevel::Medium);

        // Medium usage without LLM -> Light
        let level = CompressionLevel::auto_select(0.75, 0, 0, false);
        assert_eq!(level, CompressionLevel::Light);

        // High usage with LLM, first compression -> Heavy
        let level = CompressionLevel::auto_select(0.9, 0, 0, true);
        assert_eq!(level, CompressionLevel::Heavy);

        // High usage with LLM, already compressed -> Medium
        let level = CompressionLevel::auto_select(0.9, 3, 0, true);
        assert_eq!(level, CompressionLevel::Medium);

        // High usage with LLM failures -> Medium
        let level = CompressionLevel::auto_select(0.9, 0, 5, true);
        assert_eq!(level, CompressionLevel::Medium);
    }

    #[test]
    fn test_compress_with_level_none() {
        let messages = create_long_conversation(10);
        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.compress_with_level(&messages, CompressionLevel::None);
        assert_eq!(compressed.len(), messages.len());
    }

    #[test]
    fn test_compress_with_level_light() {
        let messages = create_long_conversation(20);
        let tokens_before = estimate_messages_tokens(&messages);
        let mut compressor = ContextCompressor::new(128_000);
        let compressed = compressor.compress_with_level(&messages, CompressionLevel::Light);
        let tokens_after = estimate_messages_tokens(&compressed);
        assert!(!compressed.is_empty());
        // Light compression trims tool results but doesn't summarize
        assert!(tokens_after <= tokens_before);
    }

    #[test]
    fn test_snip_tool_results_records_strategy() {
        let messages = vec![
            Message::tool("call_1", "x".repeat(500)),
            Message::tool("call_2", "recent"),
            Message::tool("call_3", "recent"),
            Message::tool("call_4", "recent"),
        ];
        let mut compressor = ContextCompressor::new(128_000);

        let compressed = compressor.snip_tool_results(&messages);
        let record = compressor.latest_compaction_record().unwrap();

        assert_eq!(record.strategy, ContextCompactionStrategy::Snip);
        assert_eq!(record.messages_before, messages.len());
        assert_eq!(record.messages_after, compressed.len());
        assert_eq!(record.stage_order, vec!["snip_tool_results".to_string()]);
        assert_eq!(
            record.token_delta,
            i64::try_from(record.tokens_after).unwrap()
                - i64::try_from(record.tokens_before).unwrap()
        );
        assert_eq!(record.token_pressure, Some(ContextTokenPressure::Low));
        assert!(record
            .retained_items
            .contains(&"recent_tool_results:last_3".to_string()));
        assert!(record.provenance.iter().any(|p| p == "tool_result_snip"));
    }

    #[test]
    fn test_micro_compress_records_strategy_and_provenance() {
        let messages = create_long_conversation(5);
        let mut compressor = ContextCompressor::new(128_000);

        let compressed = compressor.micro_compress(&messages);
        let record = compressor.latest_compaction_record().unwrap();

        assert_eq!(record.strategy, ContextCompactionStrategy::MicroCompact);
        assert_eq!(record.level.as_deref(), Some("light"));
        assert_eq!(record.messages_after, compressed.len());
        assert_eq!(
            record.stage_order,
            vec![
                "snip_tool_results".to_string(),
                "sanitize_tool_pairs".to_string()
            ]
        );
        assert!(record
            .retained_items
            .contains(&"tool_call_pairs:sanitized".to_string()));
        assert!(record.provenance.iter().any(|p| p == "tool_pair_sanitize"));
    }

    #[test]
    fn test_compress_with_level_medium() {
        let messages = create_long_conversation(50);
        let tokens_before = estimate_messages_tokens(&messages);
        let mut compressor = ContextCompressor::new(32_000);
        let compressed = compressor.compress_with_level(&messages, CompressionLevel::Medium);
        let tokens_after = estimate_messages_tokens(&compressed);
        assert!(!compressed.is_empty());
        assert!(
            tokens_after < tokens_before,
            "Medium compression should reduce tokens: {} -> {}",
            tokens_before,
            tokens_after
        );
    }

    #[test]
    fn test_time_based_compression_triggers() {
        use std::time::Duration;

        let config = TimeBasedConfig {
            session_duration_threshold_secs: 1, // 1 second threshold
            message_count_threshold: 5,
            ..TimeBasedConfig::default()
        };

        let mut compressor = ContextCompressor::new(128_000);
        compressor.time_config = config;

        // Create a session start time in the past
        compressor.session_start = std::time::Instant::now() - Duration::from_secs(10);

        // Should trigger time-based compression
        let messages: Vec<Message> = (0..3)
            .map(|i| Message::user(format!("Message {}", i)))
            .collect();

        assert!(compressor.needs_time_based_compression(&messages));
    }

    #[test]
    fn test_compression_warning_levels() {
        let compressor = ContextCompressor::new(100_000); // Small window

        // 50% usage - should be None or Approaching
        let low_messages = vec![
            Message::system("System"),
            Message::user("Hi"),
            Message::assistant("Hello"),
        ];

        // With small window, even few messages might approach limit
        let warning = compressor.warning_level(&low_messages);
        assert!(matches!(
            warning,
            CompressionWarning::None | CompressionWarning::Approaching
        ));
    }

    #[test]
    fn test_compact_boundary_marker() {
        let meta = CompactMetadata {
            sequence: 1,
            boundary_id: "cb-test-123".to_string(),
            preserved_tail_count: 3,
            messages_before: 20,
            messages_after: 5,
            tokens_before: 8000,
            tokens_after: 3000,
            timestamp: "2026-04-23T10:00:00+08:00".to_string(),
        };

        let marker = meta.to_boundary_marker();
        assert!(marker.contains("COMPACT_BOUNDARY"));
        assert!(marker.contains("seq=1"));
        assert!(marker.contains("id=cb-test-123"));

        // Parse it back
        let (parsed, clean) =
            CompactMetadata::parse_from_text(&format!("Summary text{}", marker)).unwrap();
        assert_eq!(parsed.sequence, 1);
        assert_eq!(parsed.boundary_id, "cb-test-123");
        assert_eq!(parsed.preserved_tail_count, 3);
        assert_eq!(parsed.messages_before, 20);
        assert_eq!(parsed.tokens_before, 8000);
        assert!(clean.starts_with("Summary text"));
    }

    #[test]
    fn test_compact_boundary_embedded_in_compression() {
        let mut compressor = ContextCompressor::new(2000);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Task 1: do something"),
            Message::assistant("Done with task 1.".to_string()),
            Message::user("Task 2: do more"),
            Message::assistant("Done with task 2.".to_string()),
            Message::user("Task 3: do even more"),
            Message::assistant("Done with task 3.".to_string()),
            Message::user("Task 4: final task"),
        ];

        let compressed = compressor.compress(&messages);

        // 应该有 compact boundary 被嵌入
        let boundaries = extract_compact_boundaries(&compressed);
        assert_eq!(boundaries.len(), 1, "Should have one compact boundary");
        assert_eq!(boundaries[0].sequence, 1);
        assert!(boundaries[0].messages_before > 0);
        assert!(boundaries[0].tokens_before > 0);

        // compressor 应该记录了历史
        assert_eq!(compressor.compact_metadata_history.len(), 1);
        let record = compressor.latest_compaction_record().unwrap();
        assert_eq!(record.strategy, ContextCompactionStrategy::AutoCompact);
        assert!(record.boundary_id.is_some());
        assert_eq!(record.sequence, Some(1));
        assert!(record
            .retained_items
            .iter()
            .any(|item| item.starts_with("tail_messages:")));
        assert!(record
            .provenance
            .iter()
            .any(|p| p.starts_with("compact_boundary:")));
    }

    #[test]
    fn test_session_memory_compact_analyze() {
        let messages = vec![
            Message::system("System prompt"),
            Message::user("Read src/main.rs and src/lib.rs"),
            Message::assistant("I read src/main.rs and src/lib.rs"),
            Message::tool("call_1", "Content of src/main.rs"),
            Message::user("Now read src/config.rs"),
            Message::assistant("I read src/config.rs and src/main.rs again"),
            Message::user("TODO: fix the bug in src/main.rs"),
        ];

        let smc = SessionMemoryCompact::analyze(&messages);

        // hot_files 应该包含出现频率高的文件
        assert!(!smc.hot_files.is_empty(), "Should detect hot files");
        assert!(
            smc.hot_files.iter().any(|f| f.contains("src/main.rs")),
            "Should detect main.rs"
        );

        // pending_tasks 应该包含 TODO
        assert!(!smc.pending_tasks.is_empty(), "Should detect pending tasks");
        assert!(
            smc.pending_tasks.iter().any(|t| t.contains("TODO")),
            "Should detect TODO"
        );
    }

    #[test]
    fn test_session_memory_compact_inject() {
        let smc = SessionMemoryCompact {
            hot_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
            user_preferences: vec!["Use concise output".to_string()],
            pending_tasks: vec!["TODO: fix bug".to_string()],
            tool_patterns: vec!["file_read".to_string()],
        };

        let mut summary = "Summary text".to_string();
        smc.inject_into_summary(&mut summary);

        assert!(summary.contains("User Preferences"));
        assert!(summary.contains("Use concise output"));
        assert!(summary.contains("Frequently Accessed Files"));
        assert!(summary.contains("src/main.rs"));
        assert!(summary.contains("Pending Tasks"));
        assert!(summary.contains("TODO: fix bug"));
        assert!(summary.contains("Common Tool Patterns"));
        assert!(summary.contains("file_read"));

        let tags = smc.provenance_tags();
        assert!(tags.contains(&"session_memory:user_preferences=1".to_string()));
        assert!(tags.contains(&"session_memory:hot_files=2".to_string()));
    }

    #[test]
    fn test_runtime_continuity_facts_extract_labeled_state() {
        let messages = vec![
            Message::assistant(
                "Active objective: finish Phase 8 compaction survivability\n\
                 Changed files: src/engine/context_compressor.rs, docs/PROJECT_STATUS.md\n\
                 Tool round: round_abc checkpoint-backed 2 file changes\n\
                 Validation passed: cargo test -q context_compressor\n\
                 Terminal task: shell_background_1 output_path=.priority-agent/tool-results/out.txt status=running\n\
                 Permission requested: bash risk_level=medium matched_rules=[git push]\n\
                 Attached context: current_diff files=src/engine/context_compressor.rs\n\
                 Diagnostics delta: improved errors -1 warnings 0\n\
                 agent_id=agent_1 task_id=task_1 status=running worktree=/tmp/agent-worktree",
            ),
            Message::user("This unrelated sentence mentions objective but is not runtime state."),
        ];

        let facts = RuntimeContinuityFacts::analyze(&messages);

        assert_eq!(facts.active_objectives.len(), 1);
        assert_eq!(facts.changed_files.len(), 1);
        assert_eq!(facts.file_change_rounds.len(), 1);
        assert_eq!(facts.validation_states.len(), 1);
        assert_eq!(facts.terminal_task_states.len(), 1);
        assert_eq!(facts.permission_states.len(), 1);
        assert_eq!(facts.context_attachments.len(), 1);
        assert_eq!(facts.diagnostic_states.len(), 1);
        assert_eq!(facts.subagent_task_states.len(), 1);
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_active_objectives:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_subagent_tasks:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_file_change_rounds:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_terminal_tasks:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_permissions:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_context_attachments:1".to_string()));
        assert!(facts
            .retained_items()
            .contains(&"runtime_state_diagnostics:1".to_string()));
    }

    #[test]
    fn test_long_task_compaction_preserves_runtime_continuity_state() {
        let mut messages = vec![
            Message::system("You are a coding agent."),
            Message::user("Please continue the release plan."),
        ];
        for i in 0..30 {
            messages.push(Message::assistant(format!(
                "Implementation note {}: repeated middle context {}",
                i,
                "x".repeat(120)
            )));
        }
        messages.push(Message::assistant(
            "Active objective: finish Phase 8 compaction survivability\n\
             Changed files: src/engine/context_compressor.rs, docs/CLAUDE_CODE_PROGRAMMING_PARITY_RELEASE_PLAN_2026-05-22.md\n\
             Tool round: round_phase8 checkpoint-backed 2 file changes\n\
             Validation passed: cargo test -q context_compressor\n\
             Terminal task: shell_background_1 output_path=.priority-agent/tool-results/out.txt status=running\n\
             Permission pending: bash risk_level=medium recovery=approve once or reject\n\
             Attached context: current_diff files=src/engine/context_compressor.rs,docs/CLAUDE_CODE_TOOL_FILE_RELIABILITY_AUDIT_PLAN_2026-05-23.md\n\
             Diagnostics: cargo check improved after src/engine/context_compressor.rs\n\
             agent_id=agent_1 task_id=task_1 status=running worktree=/tmp/agent-worktree branch=codex/agent-1234",
        ));
        for i in 30..60 {
            messages.push(Message::assistant(format!(
                "Later implementation note {}: repeated middle context {}",
                i,
                "y".repeat(120)
            )));
        }
        messages.push(Message::user("Continue from the current state."));

        let mut compressor = ContextCompressor::new(3_000);
        let compressed =
            compressor.compress_with_summary(&messages, Some("## Goal\nContinue Phase 8\n"));
        let compacted_text = compressed
            .iter()
            .map(|msg| msg.content())
            .collect::<Vec<_>>()
            .join("\n");
        let record = compressor.latest_compaction_record().unwrap();

        assert!(compacted_text.contains("## Runtime Continuity"));
        assert!(compacted_text.contains("Active objective: finish Phase 8"));
        assert!(compacted_text.contains("Changed files: src/engine/context_compressor.rs"));
        assert!(compacted_text.contains("Tool round: round_phase8"));
        assert!(compacted_text.contains("Validation passed: cargo test -q context_compressor"));
        assert!(compacted_text.contains("Terminal task: shell_background_1"));
        assert!(compacted_text.contains("Permission pending: bash"));
        assert!(compacted_text.contains("Attached context: current_diff"));
        assert!(compacted_text.contains("Diagnostics: cargo check improved"));
        assert!(compacted_text.contains("agent_id=agent_1 task_id=task_1"));
        assert!(record
            .retained_items
            .contains(&"runtime_state_active_objectives:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_changed_files:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_validation:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_file_change_rounds:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_terminal_tasks:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_permissions:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_context_attachments:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_diagnostics:1".to_string()));
        assert!(record
            .retained_items
            .contains(&"runtime_state_subagent_tasks:1".to_string()));
        assert!(record
            .provenance
            .iter()
            .any(|p| p == "summary_memory:runtime_continuity"));
        assert!(record
            .stage_order
            .contains(&"restore_runtime_continuity".to_string()));
        assert_eq!(
            record.token_delta,
            i64::try_from(record.tokens_after).unwrap()
                - i64::try_from(record.tokens_before).unwrap()
        );
    }

    #[test]
    fn test_extract_compact_boundaries_from_messages() {
        let msg1 = Message::system("Normal system message");
        let msg2 = Message::user(format!(
            "User message with boundary{}\nmore text",
            CompactMetadata {
                sequence: 2,
                boundary_id: "cb-abc".to_string(),
                preserved_tail_count: 2,
                messages_before: 10,
                messages_after: 3,
                tokens_before: 5000,
                tokens_after: 2000,
                timestamp: "2026-04-23T10:00:00+08:00".to_string(),
            }
            .to_boundary_marker()
        ));

        let boundaries = extract_compact_boundaries(&[msg1, msg2]);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].sequence, 2);
        assert_eq!(boundaries[0].boundary_id, "cb-abc");
    }
}

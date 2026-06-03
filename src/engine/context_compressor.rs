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

/// Preserved skills section appended after compression summaries.
/// Borrowed from Reasonix: skill content is extracted before compression
/// and re-appended verbatim to prevent the LLM from synthesizing/omitting it.
pub const PRESERVED_SKILLS_MARKER: &str = "\
[PRESERVED SKILLS — these are active skill definitions, preserved verbatim \
through context compression. Do not paraphrase or override them.]";

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
    messages.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(message: &Message) -> u64 {
    let content_tokens = estimate_tokens(&message.content());
    let tool_call_tokens = match message {
        Message::Assistant {
            tool_calls: Some(tool_calls),
            ..
        } if !tool_calls.is_empty() => serde_json::to_string(tool_calls)
            .map(|json| estimate_tokens(&json))
            .unwrap_or_default(),
        _ => 0,
    };
    let overhead = 4; // role, formatting 等开销
    content_tokens + tool_call_tokens + overhead
}

/// 估算工具 schema 的 token 数
pub fn estimate_tool_schemas_tokens(tools: &[crate::services::api::Tool]) -> u64 {
    crate::engine::cache_stability::provider_tool_schema_manifest(tools).estimated_tokens
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
    /// Stable prefix reused when the main agent asks the summary model to compact context.
    llm_summary_stable_prefix: Option<String>,
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
    /// Whether active skills are loaded (marker injected in summary).
    has_active_skills: bool,
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
            llm_summary_stable_prefix: None,
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
            has_active_skills: true, // skills are loaded at session start
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

    pub fn set_llm_summary_stable_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if prefix.trim().is_empty() {
            self.llm_summary_stable_prefix = None;
        } else {
            self.llm_summary_stable_prefix = Some(prefix);
        }
    }

    pub fn set_llm_summary_stable_prefix_from_messages(&mut self, messages: &[Message]) {
        if let Some(prefix) = messages.iter().find_map(|message| match message {
            Message::System { content }
                if !content.trim().is_empty()
                    && !crate::engine::cache_stability::is_dynamic_context_system_message(
                        content,
                    ) =>
            {
                Some(content.clone())
            }
            _ => None,
        }) {
            self.llm_summary_stable_prefix = Some(prefix);
        }
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

        // ── Economic guard: skip expensive compression for short conversations ──
        // Borrowed from Reasonix: don't pay LLM summarization cost when the
        // conversation is too short to benefit. Snip-only is free and sufficient.
        let message_count = messages.len();
        let is_short_conversation = message_count < 20;
        let estimated_summary_savings = if is_short_conversation {
            0.0 // Short convos gain little from summarization
        } else {
            (tokens_before as f64 * 0.3).min(8000.0) // Rough estimate: 30% of body
        };
        let skip_heavy = is_short_conversation && estimated_summary_savings < 2000.0;

        // ── Runtime diet integration: avoid re-compressing when recent
        //     compressions produced no gains. ──
        let recent_no_gain = self.consecutive_no_gain_compactions >= 2;
        if recent_no_gain && skip_heavy {
            debug!(
                "Skipping compression: {} consecutive no-gain compactions, short conversation",
                self.consecutive_no_gain_compactions
            );
            return messages.to_vec();
        }
        // ── End economic guard ──

        let level = if skip_heavy {
            // Force Medium at most — skip LLM compression for short convos
            CompressionLevel::Medium
        } else {
            CompressionLevel::auto_select(
                usage_ratio,
                self.compression_count,
                self.consecutive_llm_failures,
                self.has_llm_provider(),
            )
        };

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

        // Preserve active skills through compression (Reasonix skill-pin pattern).
        // Skills loaded by the agent are embedded in the system prompt pre-compression;
        // this marker tells the model those definitions are still active.
        if self.has_active_skills() {
            summary_text.push_str("\n\n");
            summary_text.push_str(PRESERVED_SKILLS_MARKER);
        }

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
            let tokens = estimate_message_tokens(msg);
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
        let mut summary_messages = Vec::new();
        if let Some(prefix) = self.llm_summary_stable_prefix.as_deref() {
            summary_messages.push(crate::services::api::Message::system(prefix));
        }
        summary_messages.push(crate::services::api::Message::user(&prompt));

        let request =
            crate::services::api::ChatRequest::new(&self.llm_model).with_messages(summary_messages);

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

    /// Whether active skills were loaded this session (marker injected in summaries).
    pub fn has_active_skills(&self) -> bool {
        self.has_active_skills
    }

    /// Mark that skills are active so the compression pipeline preserves them.
    pub fn mark_skills_active(&mut self) {
        self.has_active_skills = true;
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
mod tests;

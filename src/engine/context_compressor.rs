//! 上下文压缩器
//!
//! 参考 hermes-agent 的设计：
//! - Token 预算管理（根据模型上下文窗口动态计算）
//! - 两阶段压缩：先裁剪工具输出，再 LLM 摘要
//! - 8 段结构化摘要模板（Goal/Constraints/Progress/Decisions/Files/Next Steps/Critical Context/Tools & Patterns）
//! - 迭代式摘要更新（累积知识而非丢失）
//! - Token-budget 尾部保护（soft_ceiling = budget * 1.5）
//! - 工具调用对完整性校验（孤立项清理 + stub 插入）
//!
//! ## 压缩路径职责边界
//!
//! 当前有多条压缩/修复路径，它们的职责不同：
//!
//! | 路径 | 位置 | 触发时机 | 改变历史？ | 记录边界？ |
//! |------|------|---------|-----------|-----------|
//! | **Full-message compaction** | `ContextCompressor` (preflight) | token 压力 > 80% | ✅ | ✅ `compact_boundary` |
//! | **Streaming pre-query** | `streaming.rs` | 每次 query 前 | ✅ | 通过 `ContextCompressor` |
//! | **API reactive compaction** | `api_request_controller` | provider 返回 context limit | ✅ | ✅ `CompactionRuntimeRecord` |
//! | **Selective tool-output** | `message_compression` | 每轮 request preparation | ❌ (仅本轮) | ❌ |
//! | **Message healing** | `message_healing` | 发送到 provider 前 | ❌ (仅本轮) | ❌ |
//!
//! - 前三条路径会改变持久化消息历史，后两条只影响本次 request。
//! - `message_healing` 不属于语义压缩，但对可发送性至关重要。
//! - `ContextCollapseService` 是磁盘折叠的实验性替代路径，当前未接入主运行时。

pub use crate::engine::context_collapse::{
    extract_compact_boundaries, CompactMetadata, CompactionAttemptRecord, CompactionDecision,
    CompactionRuntimeRecord, ContextCompactionStrategy, ContextTokenPressure,
};
use crate::services::api::Message;
#[cfg(test)]
use crate::services::api::ToolCall;
use tracing::{debug, info, warn};
use uuid::Uuid;

mod compressor;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenEstimateProfile {
    GeneralText,
    JsonToolSchema,
    CjkHeavy,
}

impl TokenEstimateProfile {
    pub fn for_model_context(profile: &crate::engine::model_context::ModelContextProfile) -> Self {
        use crate::services::api::provider_protocol::ProviderProtocolFamily;

        match profile.provider_family {
            ProviderProtocolFamily::MiniMax | ProviderProtocolFamily::Kimi => Self::CjkHeavy,
            ProviderProtocolFamily::AnthropicLike
            | ProviderProtocolFamily::ReasoningCapable
            | ProviderProtocolFamily::OpenAiCompatible => Self::GeneralText,
        }
    }
}

/// 默认 token 估算，面向普通混合文本。
pub fn estimate_tokens(text: &str) -> u64 {
    estimate_tokens_for_profile(text, TokenEstimateProfile::GeneralText)
}

pub fn estimate_tokens_for_model_context(
    text: &str,
    profile: &crate::engine::model_context::ModelContextProfile,
) -> u64 {
    estimate_tokens_for_profile(text, TokenEstimateProfile::for_model_context(profile))
}

pub fn estimate_tokens_for_profile(text: &str, profile: TokenEstimateProfile) -> u64 {
    if text.is_empty() {
        return 0;
    }

    let mut ascii_word = 0u64;
    let mut ascii_whitespace = 0u64;
    let mut ascii_punct = 0u64;
    let mut cjk_chars = 0u64;
    let mut other_unicode_bytes = 0u64;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            ascii_word += 1;
        } else if ch.is_ascii_whitespace() {
            ascii_whitespace += 1;
        } else if ch.is_ascii() {
            ascii_punct += 1;
        } else if is_cjk_char(ch) {
            cjk_chars += 1;
        } else {
            other_unicode_bytes += ch.len_utf8() as u64;
        }
    }

    let ascii_word_tokens = ascii_word.div_ceil(4);
    let whitespace_tokens = ascii_whitespace.div_ceil(16);
    let ascii_punct_tokens = match profile {
        TokenEstimateProfile::JsonToolSchema => ascii_punct,
        TokenEstimateProfile::GeneralText | TokenEstimateProfile::CjkHeavy => {
            ascii_punct.div_ceil(3)
        }
    };
    let cjk_tokens = match profile {
        TokenEstimateProfile::CjkHeavy => cjk_chars.saturating_mul(2),
        TokenEstimateProfile::GeneralText | TokenEstimateProfile::JsonToolSchema => {
            cjk_chars.saturating_mul(3).div_ceil(2)
        }
    };
    let other_unicode_tokens = other_unicode_bytes.div_ceil(2);

    ascii_word_tokens
        .saturating_add(whitespace_tokens)
        .saturating_add(ascii_punct_tokens)
        .saturating_add(cjk_tokens)
        .saturating_add(other_unicode_tokens)
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x30000..=0x3134F
    )
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
            .map(|json| estimate_tokens_for_profile(&json, TokenEstimateProfile::JsonToolSchema))
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

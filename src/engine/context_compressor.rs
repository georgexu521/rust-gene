//! 上下文压缩器
//!
//! 参考 hermes-agent 的设计：
//! - Token 预算管理（根据模型上下文窗口动态计算）
//! - 两阶段压缩：先裁剪工具输出，再 LLM 摘要
//! - 8 段结构化摘要模板（Goal/Constraints/Progress/Decisions/Files/Next Steps/Critical Context/Tools & Patterns）
//! - 迭代式摘要更新（累积知识而非丢失）
//! - Token-budget 尾部保护（soft_ceiling = budget * 1.5）
//! - 工具调用对完整性校验（孤立项清理 + stub 插入）

use crate::services::api::Message;
#[cfg(test)]
use crate::services::api::ToolCall;
use tracing::{debug, info, warn};

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

impl TokenBudget {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            max_context_tokens,
            reserved_output_tokens: 4096,
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

// ── 上下文压缩器 ──────────────────────────────────────────

/// 上下文压缩器
pub struct ContextCompressor {
    budget: TokenBudget,
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
}

impl ContextCompressor {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            budget: TokenBudget::new(max_context_tokens),
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
        }
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

    /// 异步压缩消息列表
    /// 优先尝试 LLM 生成高质量摘要，失败时回退到启发式摘要
    pub async fn compress_async(&mut self, messages: &[Message]) -> Vec<Message> {
        let has_provider = self.has_llm_provider();
        let tokens_before = estimate_messages_tokens(messages);
        self.total_tokens_before += tokens_before;

        let result = if has_provider && !self.is_in_cooldown() {
            self.llm_compression_attempts += 1;
            match self.llm_summarize_middle(messages).await {
                Some(summary_text) => {
                    let compressed = self.compress_with_summary(messages, Some(&summary_text));
                    let tokens_after = estimate_messages_tokens(&compressed);
                    self.total_tokens_after += tokens_after;
                    info!(
                        "LLM compression succeeded: {} -> {} tokens (saved {}%)",
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
                    self.record_failure();
                    let compressed = self.compress(messages);
                    let tokens_after = estimate_messages_tokens(&compressed);
                    self.total_tokens_after += tokens_after;
                    warn!(
                        "LLM compression failed, fell back to heuristic: {} -> {} tokens",
                        tokens_before, tokens_after
                    );
                    compressed
                }
            }
        } else {
            let compressed = self.compress(messages);
            let tokens_after = estimate_messages_tokens(&compressed);
            self.total_tokens_after += tokens_after;
            compressed
        };

        result
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
        if messages.is_empty() {
            return messages.to_vec();
        }

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
        let summary_text = if let Some(text) = summary_text {
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

        // Phase 4: 组装结果
        let mut result = head.to_vec();

        if !summary_text.is_empty() {
            let formatted_summary = if self.compression_count > 0 {
                format!(
                    "{}\n（上下文已压缩 {} 次，保留累积知识）\n\n{}",
                    SUMMARY_PREFIX,
                    self.compression_count + 1,
                    summary_text
                )
            } else {
                format!("{}\n\n{}", SUMMARY_PREFIX, summary_text)
            };

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

        self.compression_count += 1;

        info!("Compressed to {} messages", result.len());
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
                            format!("{}...(truncated)", &content[..keep_len])
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
                            if let Some(cmd) =
                                tc.arguments["command"].as_str().or_else(|| tc.arguments["cmd"].as_str())
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
                    if content.contains("error") || content.contains("Error") {
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
                            || trimmed.contains("token")
                            || trimmed.contains("secret")
                            || trimmed.contains("password")
                            || trimmed.contains("error")
                            || trimmed.contains("failed")
                            || trimmed.contains("diagnostic")
                            || trimmed.contains("path"))
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
            &conversation[..conversation.len().min(8000)]
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
        let mut messages = vec![
            Message::user("start"),
            Message::assistant("ok"),
        ];
        for i in 0..6 {
            let content = if i == 0 {
                format!(
                    "Result: ERROR\n{}\n",
                    "x".repeat(1500)
                )
            } else {
                "Result: OK\nsmall output".to_string()
            };
            messages.push(Message::tool(format!("call_{}", i), content));
        }

        let pruned = ContextCompressor::prune_old_tool_results(&messages);
        let first_tool = pruned
            .iter()
            .find_map(|m| match m {
                Message::Tool { tool_call_id, content } if tool_call_id == "call_0" => {
                    Some(content.clone())
                }
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

        let mut compressor = ContextCompressor::new(1000)
            .with_llm_provider(provider, "mock-model");

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
        assert!(has_summary, "LLM compression should produce a summary message");

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

        let mut compressor = ContextCompressor::new(1000)
            .with_llm_provider(provider, "mock-model");

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
}

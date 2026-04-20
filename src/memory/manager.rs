//! 记忆管理器
//!
//! 参考 hermes-agent 的 MemoryManager 设计：
//! - 冻结快照：会话开始时冻结记忆，中间写入不 bust prompt cache
//! - 预取：每轮对话前搜索相关记忆注入上下文
//! - 同步：每轮结束后自动提取关键信息保存
//! - 会话结束提取：session 过期时批量提取学习内容

use crate::services::api::{ChatRequest, LlmProvider, Message};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

const MAX_LEARNINGS_PER_TURN: usize = 3;
const MAX_LEARNINGS_PER_SESSION_EXTRACT: usize = 6;

/// 记忆条目
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub content: String,
    pub category: String,
    pub timestamp: String,
}

/// 记忆管理器
pub struct MemoryManager {
    /// MEMORY.md 路径
    memory_path: PathBuf,
    /// USER.md 路径（用户偏好）
    user_path: PathBuf,
    /// 冻结快照（会话开始时捕获，整个会话不变）
    frozen_memory: Option<String>,
    frozen_user: Option<String>,
    /// 字符限制
    memory_char_limit: usize,
    user_char_limit: usize,
    /// 本轮是否已预取
    prefetched_this_turn: bool,
    /// 累积的学习内容（会话结束时批量保存）
    pending_learnings: Vec<String>,
    /// 已记录的学习内容哈希（去重）
    seen_hashes: HashSet<u64>,
    /// 本会话轮数（用于 throttle LLM 提取）
    turn_count: usize,
    /// 上次 LLM 提取的轮数
    last_llm_extraction_turn: usize,
    /// LLM 提取次数（用于 telemetry）
    llm_extraction_count: usize,
    /// 主 agent 已写入标记（mutual exclusion）
    main_agent_wrote_this_turn: bool,
}

impl MemoryManager {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent");

        let _ = std::fs::create_dir_all(&base);

        Self {
            memory_path: base.join("MEMORY.md"),
            user_path: base.join("USER.md"),
            frozen_memory: None,
            frozen_user: None,
            memory_char_limit: 3000,
            user_char_limit: 1500,
            prefetched_this_turn: false,
            pending_learnings: Vec::new(),
            seen_hashes: HashSet::new(),
            turn_count: 0,
            last_llm_extraction_turn: 0,
            llm_extraction_count: 0,
            main_agent_wrote_this_turn: false,
        }
    }

    /// 会话开始时冻结快照（同步版本 — 兼容非异步上下文）
    pub fn freeze_snapshot(&mut self) {
        self.frozen_memory = std::fs::read_to_string(&self.memory_path).ok();
        self.frozen_user = std::fs::read_to_string(&self.user_path).ok();
        info!("Memory snapshot frozen for this session");
    }

    /// 会话开始时冻结快照（异步版本 — 推荐在异步上下文中使用）
    pub async fn freeze_snapshot_async(&mut self) {
        self.frozen_memory = tokio::fs::read_to_string(&self.memory_path).await.ok();
        self.frozen_user = tokio::fs::read_to_string(&self.user_path).await.ok();
        info!("Memory snapshot frozen for this session (async)");
    }

    /// 获取冻结的快照（用于 system prompt 注入）
    pub fn get_snapshot(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref mem) = self.frozen_memory {
            let trimmed = mem.trim();
            if !trimmed.is_empty() {
                let truncated: String = trimmed.chars().take(self.memory_char_limit).collect();
                parts.push(format!("## Project Memory\n{}", truncated));
            }
        }

        if let Some(ref user) = self.frozen_user {
            let trimmed = user.trim();
            if !trimmed.is_empty() {
                let truncated: String = trimmed.chars().take(self.user_char_limit).collect();
                parts.push(format!("## User Preferences\n{}", truncated));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            // XML 围栏包裹，防止模型将记忆上下文视为用户输入
            format!(
                "<memory-context>\n{}\n</memory-context>\n",
                parts.join("\n\n")
            )
        }
    }

    /// 预取：根据当前用户消息搜索相关记忆
    pub fn prefetch(&mut self, user_message: &str) -> String {
        if self.prefetched_this_turn {
            return String::new();
        }
        self.prefetched_this_turn = true;

        // 从冻结快照中搜索（而非磁盘），保持一致性
        let memory_content = self.frozen_memory.clone().unwrap_or_default();
        if memory_content.trim().is_empty() {
            return String::new();
        }

        // 简单关键词匹配搜索相关段落
        let keywords = extract_keywords(user_message);
        let relevant = search_memory(&memory_content, &keywords, 3);

        if relevant.is_empty() {
            String::new()
        } else {
            let entries: Vec<String> = relevant
                .into_iter()
                .map(|e| format!("- {}", e.trim()))
                .collect();
            format!("[Relevant Memory]\n{}\n\n---\n", entries.join("\n"))
        }
    }

    /// 同步：保存本轮对话中学习到的内容（启发式提取）
    pub fn sync_turn(&mut self, user: &str, assistant: &str) {
        let learnings = extract_learnings_from_turn(user, assistant);
        self.ingest_learnings(learnings, MAX_LEARNINGS_PER_TURN);
    }

    /// 同步：保存本轮对话中学习到的内容（支持 LLM 增强提取）
    pub async fn sync_turn_llm(
        &mut self,
        user: &str,
        assistant: &str,
        provider: Option<&dyn LlmProvider>,
        model: &str,
    ) {
        // 先尝试启发式提取
        let heuristic = extract_learnings_from_turn(user, assistant);
        self.ingest_learnings(heuristic.clone(), MAX_LEARNINGS_PER_TURN);

        // 若启发式无结果且启用了 LLM 提取，则调用 LLM
        if heuristic.is_empty() {
            if let Some(p) = provider {
                let llm_learnings = self
                    .extract_memories_with_llm(user, assistant, p, model)
                    .await;
                self.ingest_learnings(llm_learnings, MAX_LEARNINGS_PER_TURN);
            }
        }
    }

    /// 后台 LLM 记忆提取（不阻塞主对话循环）
    ///
    /// 使用 `spawn` 在后台 fork 一个 task 进行 LLM 调用，
    /// 主对话循环不会被 LLM 延迟阻塞。
    pub fn sync_turn_llm_background(
        &self,
        user: String,
        assistant: String,
        provider: Arc<dyn LlmProvider>,
        model: String,
    ) {
        let path = self.memory_path.clone();

        tokio::spawn(async move {
            // 小延迟，让主对话先完成响应
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let heuristic = extract_learnings_from_turn(&user, &assistant);
            if heuristic.is_empty() {
                let system_prompt = "You are a memory extraction assistant. \
Analyze the conversation turn and extract up to 3 concise memory bullets of CRITICAL CONTEXT only. \
Critical context includes: API keys or paths, architecture decisions, user preferences, \
specific error messages and their fixes, project conventions, or important configuration values. \
Each bullet should be one line starting with '- '. \
Return exactly the word NONE if there is nothing critical to remember.";

                let content = format!(
                    "User:\n{}\n\nAssistant:\n{}\n",
                    user,
                    assistant.chars().take(4000).collect::<String>()
                );

                let request = ChatRequest::new(&model).with_messages(vec![
                    Message::system(system_prompt),
                    Message::user(&content),
                ]);

                if let Ok(response) = provider.chat(request).await {
                    let text = response.content.trim();
                    if !text.eq_ignore_ascii_case("NONE") && !text.is_empty() {
                        let bullets: Vec<String> = text
                            .lines()
                            .map(|l: &str| l.trim())
                            .filter(|l| !l.is_empty())
                            .map(|l| {
                                if let Some(stripped) = l.strip_prefix("- ") {
                                    stripped.to_string()
                                } else {
                                    l.to_string()
                                }
                            })
                            .filter(|l| !l.is_empty())
                            .collect();
                        debug!("Background LLM extracted {} memory bullets", bullets.len());

                        // 写入文件（不依赖 MemoryManager 内部状态）
                        for bullet in bullets {
                            let entry = format!(
                                "- [{}] {}\n",
                                chrono::Local::now().format("%Y-%m-%d %H:%M"),
                                bullet
                            );
                            let existing = std::fs::read_to_string(&path).unwrap_or_default();
                            let new_content = format!("{}{}", existing, entry);
                            let _ = std::fs::write(&path, new_content);
                        }
                    }
                }
            }
        });
    }

    /// 使用 LLM 从对话中提取记忆
    async fn extract_memories_with_llm(
        &self,
        user: &str,
        assistant: &str,
        provider: &dyn LlmProvider,
        model: &str,
    ) -> Vec<String> {
        let system_prompt = "You are a memory extraction assistant. \
Analyze the conversation turn and extract up to 3 concise memory bullets of CRITICAL CONTEXT only. \
Critical context includes: API keys or paths, architecture decisions, user preferences, specific error messages and their fixes, project conventions, or important configuration values. \
Each bullet should be one line starting with '- '. \
Return exactly the word NONE if there is nothing critical to remember.";

        let content = format!(
            "User:\n{}\n\nAssistant:\n{}\n",
            user,
            assistant.chars().take(4000).collect::<String>()
        );

        let request = ChatRequest::new(model).with_messages(vec![
            crate::services::api::Message::system(system_prompt),
            crate::services::api::Message::user(&content),
        ]);

        match provider.chat(request).await {
            Ok(response) => {
                let text = response.content.trim();
                if text.eq_ignore_ascii_case("NONE") || text.is_empty() {
                    return Vec::new();
                }
                let bullets: Vec<String> = text
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .map(|l| {
                        // 统一去掉开头的 '- '
                        if let Some(stripped) = l.strip_prefix("- ") {
                            stripped.to_string()
                        } else {
                            l.to_string()
                        }
                    })
                    .filter(|l| !l.is_empty())
                    .collect();
                debug!("LLM extracted {} memory bullets", bullets.len());
                bullets
            }
            Err(e) => {
                warn!("LLM memory extraction failed: {}", e);
                Vec::new()
            }
        }
    }

    /// 添加学习内容（同步版本）
    pub fn add_learning(&mut self, content: &str, category: &str) {
        let path = match category {
            "preference" | "user" => &self.user_path,
            _ => &self.memory_path,
        };

        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let normalized_existing = existing.to_lowercase().replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
        let normalized_content = content.to_lowercase().replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
        if normalized_existing.contains(&normalized_content) {
            debug!("Skipping duplicate learning (already in file): {}", &content[..content.len().min(50)]);
            return;
        }

        let entry = format!(
            "\n## [{}] {}\n{}\n",
            category.to_uppercase(),
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            content
        );

        let header = if existing.trim().is_empty() {
            if path == &self.user_path {
                "# User Preferences\n".to_string()
            } else {
                "# Priority Agent Memory\n".to_string()
            }
        } else {
            String::new()
        };

        let new_content = format!("{}{}{}", existing, header, entry);
        let _ = std::fs::write(path, &new_content);

        debug!(
            "Memory saved: [{}] {}",
            category,
            &content[..content.len().min(50)]
        );
    }

    /// 添加学习内容（异步版本 — 推荐在异步上下文中使用）
    pub async fn add_learning_async(&self, content: &str, category: &str) {
        let path = match category {
            "preference" | "user" => &self.user_path,
            _ => &self.memory_path,
        };

        let existing = tokio::fs::read_to_string(path).await.unwrap_or_default();
        let normalized_existing = existing.to_lowercase().replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
        let normalized_content = content.to_lowercase().replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
        if normalized_existing.contains(&normalized_content) {
            debug!("Skipping duplicate learning (already in file, async): {}", &content[..content.len().min(50)]);
            return;
        }

        let entry = format!(
            "\n## [{}] {}\n{}\n",
            category.to_uppercase(),
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            content
        );

        let header = if existing.trim().is_empty() {
            if path == &self.user_path {
                "# User Preferences\n".to_string()
            } else {
                "# Priority Agent Memory\n".to_string()
            }
        } else {
            String::new()
        };

        let new_content = format!("{}{}{}", existing, header, entry);
        let _ = tokio::fs::write(path, &new_content).await;

        debug!(
            "Memory saved (async): [{}] {}",
            category,
            &content[..content.len().min(50)]
        );
    }

    /// 会话结束时批量提取学习内容（同步版本）
    pub fn flush_session(&mut self, messages: &[Message]) {
        let session_learnings = extract_session_learnings(messages);
        self.ingest_learnings(session_learnings, MAX_LEARNINGS_PER_SESSION_EXTRACT);

        let pending: Vec<String> = self.pending_learnings.drain(..).collect();
        if !pending.is_empty() {
            info!("Flushing {} learnings from session", pending.len());
            for learning in &pending {
                self.add_learning(learning, "learned");
            }
        }
    }

    /// 会话结束时批量提取学习内容（异步版本）
    pub async fn flush_session_async(&mut self, messages: &[Message]) {
        let session_learnings = extract_session_learnings(messages);
        self.ingest_learnings(session_learnings, MAX_LEARNINGS_PER_SESSION_EXTRACT);

        let pending: Vec<String> = self.pending_learnings.drain(..).collect();
        if !pending.is_empty() {
            info!("Flushing {} learnings from session (async)", pending.len());
            for learning in &pending {
                self.add_learning_async(learning, "learned").await;
            }
        }
    }

    /// 重置预取状态（每轮开始时调用）
    pub fn reset_turn(&mut self) {
        self.prefetched_this_turn = false;
        self.main_agent_wrote_this_turn = false;
    }

    /// 本轮结束，增加轮数计数
    pub fn increment_turn(&mut self) {
        self.turn_count += 1;
    }

    /// 获取 LLM 提取间隔（环境变量可配置）
    pub fn llm_extraction_interval() -> usize {
        std::env::var("PRIORITY_AGENT_LLM_MEMORY_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5)
    }

    /// 获取 telemetry 统计
    pub fn extraction_stats(&self) -> (usize, usize, usize) {
        (self.llm_extraction_count, self.turn_count, self.last_llm_extraction_turn)
    }

    /// 主 agent 已写入，阻止后台 LLM 提取
    pub fn mark_main_agent_wrote(&mut self) {
        self.main_agent_wrote_this_turn = true;
    }

    /// 检查是否应进行 LLM 提取（throttle + mutual exclusion）
    pub fn should_extract_with_llm(&self) -> bool {
        // mutual exclusion：主 agent 已写则跳过
        if self.main_agent_wrote_this_turn {
            return false;
        }
        // throttle：每 N 轮提取一次
        let interval = Self::llm_extraction_interval();
        self.turn_count - self.last_llm_extraction_turn >= interval
    }

    /// 检查内容是否已重复
    pub fn is_duplicate(&self, content: &str) -> bool {
        let hash = hash_learning(content);
        self.seen_hashes.contains(&hash)
    }

    /// 搜索记忆
    pub fn search(&self, query: &str) -> Vec<String> {
        let memory_content = std::fs::read_to_string(&self.memory_path).unwrap_or_default();
        let keywords = extract_keywords(query);
        search_memory(&memory_content, &keywords, 5)
    }

    /// 尝试添加学习内容到 pending，去重
    fn push_learning(&mut self, content: String) {
        let content = content.trim();
        if !Self::passes_quality_gate(content) {
            debug!(
                "Skip low-signal memory candidate: {}",
                &content[..content.len().min(60)]
            );
            return;
        }
        let hash = hash_learning(content);
        if self.seen_hashes.insert(hash) {
            self.pending_learnings.push(content.to_string());
        }
    }

    fn ingest_learnings(&mut self, learnings: Vec<String>, max_items: usize) {
        let mut accepted = 0usize;
        for learning in learnings {
            let before = self.pending_learnings.len();
            self.push_learning(learning);
            if self.pending_learnings.len() > before {
                accepted += 1;
            }
            if accepted >= max_items {
                break;
            }
        }
    }

    fn passes_quality_gate(content: &str) -> bool {
        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed.len() < 16 || trimmed.len() > 600 {
            return false;
        }

        let lower = trimmed.to_lowercase();
        let low_signal_phrases = [
            "thank you",
            "thanks",
            "okay",
            "ok",
            "got it",
            "hope this helps",
            "好的",
            "谢谢",
            "明白",
            "可以继续",
            "已完成",
            "done",
        ];
        if trimmed.len() < 90 && low_signal_phrases.iter().any(|p| lower.contains(p)) {
            return false;
        }

        let strong_prefix = [
            "User preference:",
            "Solution:",
            "Lesson:",
            "Frequently used tool:",
            "Successful task pattern:",
        ];
        if strong_prefix.iter().any(|p| trimmed.starts_with(p)) {
            return true;
        }

        let signal_markers = [
            ".rs",
            ".toml",
            ".md",
            "cargo ",
            "rust",
            "tool",
            "agent",
            "memory",
            "token",
            "error",
            "fix",
            "prefer",
            "preference",
            "偏好",
            "配置",
            "路径",
            "/",
        ];

        signal_markers.iter().any(|m| lower.contains(m))
    }

    /// 获取待保存的学习内容数量
    pub fn pending_count(&self) -> usize {
        self.pending_learnings.len()
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 归一化学习内容并计算哈希（用于去重）
fn hash_learning(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let normalized = text.to_lowercase().trim().replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
}

/// 从文本中提取关键词
fn extract_keywords(text: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上", "也",
        "很", "到", "说", "要", "去", "你", "会", "着", "the", "a", "an", "is", "are", "was",
        "were", "be", "been", "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "i", "you", "he", "she", "it", "we", "they", "this", "that", "what",
    ]
    .iter()
    .cloned()
    .collect();

    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() >= 2 && !stop_words.contains(w.to_lowercase().as_str()))
        .map(|w| w.to_lowercase())
        .collect()
}

/// 从记忆文件中搜索相关段落
fn search_memory(content: &str, keywords: &[String], max_results: usize) -> Vec<String> {
    if keywords.is_empty() || content.trim().is_empty() {
        return Vec::new();
    }

    // 按段落分割（以 ## 开头或空行为界）
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for line in content.lines() {
        if line.starts_with("## ") || (line.trim().is_empty() && !current.trim().is_empty()) {
            if !current.trim().is_empty() {
                paragraphs.push(current.clone());
            }
            current = if line.starts_with("## ") {
                line.to_string()
            } else {
                String::new()
            };
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        paragraphs.push(current);
    }

    // 按关键词匹配度排序
    let mut scored: Vec<(usize, String)> = paragraphs
        .into_iter()
        .map(|p| {
            let p_lower = p.to_lowercase();
            let score = keywords
                .iter()
                .filter(|k| p_lower.contains(k.as_str()))
                .count();
            (score, p)
        })
        .filter(|(score, _)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(max_results)
        .map(|(_, content)| content)
        .collect()
}

/// 从单轮对话中提取学习内容
fn extract_learnings_from_turn(user: &str, assistant: &str) -> Vec<String> {
    let mut learnings = Vec::new();

    // 检测用户偏好信号
    let user_lower = user.to_lowercase();
    if user_lower.contains("我喜欢")
        || user_lower.contains("i prefer")
        || user_lower.contains("我更喜欢")
    {
        learnings.push(format!("User preference: {}", user));
    }

    // 检测问题解决模式
    let assistant_lower = assistant.to_lowercase();
    if assistant_lower.contains("解决方案")
        || assistant_lower.contains("solution")
        || assistant_lower.contains("修复方法")
        || assistant_lower.contains("workaround")
    {
        // 提取解决方案段落
        for line in assistant.lines() {
            let line_lower = line.to_lowercase();
            if (line_lower.contains("解决")
                || line_lower.contains("fix")
                || line_lower.contains("方法")
                || line_lower.contains("approach"))
                && line.len() > 20
                && line.len() < 500
            {
                learnings.push(format!("Solution: {}", line.trim()));
            }
        }
    }

    // 检测错误和教训
    if assistant_lower.contains("error")
        || assistant_lower.contains("错误")
        || assistant_lower.contains("失败")
        || assistant_lower.contains("failed")
    {
        for line in assistant.lines() {
            if line.len() > 30
                && line.len() < 300
                && (line.to_lowercase().contains("error") || line.contains("错误"))
            {
                learnings.push(format!("Lesson: {}", line.trim()));
            }
        }
    }

    learnings
}

/// 从完整会话中提取学习内容
fn extract_session_learnings(messages: &[Message]) -> Vec<String> {
    let mut learnings = Vec::new();

    // 统计工具使用频率
    let mut tool_usage: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for msg in messages {
        if let Message::Assistant {
            tool_calls: Some(calls),
            ..
        } = msg
        {
            for tc in calls {
                *tool_usage.entry(tc.name.clone()).or_insert(0) += 1;
            }
        }
    }

    // 记录高频工具
    for (tool, count) in &tool_usage {
        if *count >= 3 {
            learnings.push(format!("Frequently used tool: {} ({} times)", tool, count));
        }
    }

    // 检测成功的模式
    let all_content: String = messages
        .iter()
        .filter_map(|m| match m {
            Message::Assistant { content, .. } => Some(content.as_str()),
            Message::Tool { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    if all_content.contains("✅") || all_content.contains("success") || all_content.contains("完成")
    {
        // 找到成功的上下文
        for msg in messages.iter().rev().take(5) {
            if let Message::User { content } = msg {
                if content.len() > 20 && content.len() < 200 {
                    learnings.push(format!("Successful task pattern: {}", content.trim()));
                    break;
                }
            }
        }
    }

    learnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords("How do I implement authentication in Rust?");
        assert!(keywords.contains(&"implement".to_string()));
        assert!(keywords.contains(&"authentication".to_string()));
        assert!(keywords.contains(&"rust".to_string()));
        assert!(!keywords.contains(&"do".to_string())); // stop word
    }

    #[test]
    fn test_search_memory() {
        let content = r#"# Memory

## Project Conventions
Use snake_case for Rust functions.

## API Notes
The auth endpoint requires JWT tokens.

## Debugging Tips
Always check logs first.
"#;
        let keywords = vec!["auth".to_string(), "jwt".to_string()];
        let results = search_memory(content, &keywords, 3);
        assert!(!results.is_empty());
        assert!(results[0].contains("auth"));
    }

    #[test]
    fn test_extract_learnings() {
        let learnings = extract_learnings_from_turn(
            "I prefer using async/await",
            "Sure, here's the solution using async/await...",
        );
        assert!(!learnings.is_empty());
    }

    #[test]
    fn test_frozen_snapshot() {
        let mut mgr = MemoryManager::new();
        mgr.freeze_snapshot();
        let snapshot = mgr.get_snapshot();
        // 无记忆文件时应返回空
        assert!(snapshot.is_empty() || snapshot.contains("[Memory Context]"));
    }

    #[test]
    fn test_deduplication_in_pending() {
        let mut mgr = MemoryManager::new();
        mgr.sync_turn("I prefer async/await", "Solution using async/await...");
        let first_count = mgr.pending_count();
        assert!(first_count > 0);

        // 同一内容再次同步，不应增加
        mgr.sync_turn("I prefer async/await", "Solution using async/await...");
        assert_eq!(mgr.pending_count(), first_count);
    }

    #[test]
    fn test_is_duplicate() {
        let mut mgr = MemoryManager::new();
        mgr.push_learning("User prefers dark mode".to_string());
        assert!(mgr.is_duplicate("User prefers dark mode"));
        assert!(!mgr.is_duplicate("User prefers light mode"));
    }

    #[test]
    fn test_quality_gate_filters_low_signal_memory() {
        let mut mgr = MemoryManager::new();
        mgr.push_learning("好的，谢谢".to_string());
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn test_quality_gate_keeps_structured_memory() {
        let mut mgr = MemoryManager::new();
        mgr.push_learning("Solution: Use cargo check before cargo test to fail fast.".to_string());
        assert_eq!(mgr.pending_count(), 1);
    }

    #[test]
    fn test_should_extract_with_llm_throttled() {
        let mut mgr = MemoryManager::new();
        // 首轮不应提取（last_llm_extraction_turn = 0，turn_count = 0，interval = 5）
        assert!(!mgr.should_extract_with_llm());

        // 轮数未到 interval，不应提取
        for i in 1..5 {
            mgr.increment_turn();
            assert!(!mgr.should_extract_with_llm(), "turn {} should not trigger", i);
        }

        // 第 5 轮应该触发
        mgr.increment_turn();
        assert!(mgr.should_extract_with_llm());
    }

    #[test]
    fn test_mutual_exclusion_main_agent_wrote() {
        let mut mgr = MemoryManager::new();

        // 触发 throttle：需要 turn_count >= interval (5)
        for _ in 0..5 {
            mgr.increment_turn();
        }

        // 主 agent 未写时，throttled 提取可触发
        assert!(mgr.should_extract_with_llm(), "should trigger when throttled");

        // 主 agent 写入后，阻止后台 LLM 提取（mutual exclusion）
        mgr.mark_main_agent_wrote();
        assert!(!mgr.should_extract_with_llm(), "main agent wrote blocks extraction");
    }

    #[test]
    fn test_llm_extraction_interval_env_var() {
        // 默认是 5
        assert_eq!(MemoryManager::llm_extraction_interval(), 5);
    }

    #[test]
    fn test_extraction_stats() {
        let mut mgr = MemoryManager::new();
        mgr.increment_turn();
        mgr.increment_turn();
        mgr.increment_turn();

        let (count, turns, last) = mgr.extraction_stats();
        assert_eq!(count, 0); // 尚未触发 LLM 提取
        assert_eq!(turns, 3);
        assert_eq!(last, 0);
    }
}

//! 后台审查：独立 LLM 调用审查对话历史，自动判断是否值得记住。
//!
//! 不同于 `extraction.rs` 的逐轮提取，后台审查由 nudge 计数器触发（默认 10 轮
//! 未调 memory 工具后），用专用 prompt 让 LLM 审视多轮对话中值得保存的信息。

use super::manager::{memory_llm_timeout, MemoryManager, MemoryWriteTarget};
use super::types::MemoryProvenance;
use crate::services::api::{ChatRequest, LlmProvider, Message};
use tracing::debug;

const BACKGROUND_REVIEW_PROMPT: &str = r#"Review the conversation below between a user and an AI coding assistant.

1. Has the user revealed personal details worth remembering (name, role, preferences,
   coding style, communication style)?
2. Has the user expressed expectations about the assistant's behavior that should
   carry forward to future sessions?
3. Have you identified project conventions, environment facts, or tool quirks that
   will be useful in future sessions?
4. Has the user corrected the assistant? (These are especially important to remember.)

If any of the above is true, return a JSON object with memory candidates:
{"memory_candidates":[{"type":"project_fact|user_preference|strategy|failure_lesson|note","content":"...","confidence":0.0,"importance":1,"evidence":"..."}]}

Do NOT save task progress, arbitrary code excerpts, temporary state, or things the
user said offhand without intent to be remembered.
If there is genuinely nothing worth remembering, return exactly the word NONE."#;

impl MemoryManager {
    /// 后台审查：独立的 LLM 调用，审查对话历史是否有值得保存的记忆。
    ///
    /// 会在内部设置 `background_review_active` 防重入，完成后自动清除。
    /// 不阻塞调用方——应由 tokio::spawn 包装。
    pub async fn run_background_review(
        &mut self,
        user: &str,
        assistant: &str,
        provider: &dyn LlmProvider,
        model: &str,
    ) {
        if self.background_review_active {
            debug!("Background review already active, skipping");
            return;
        }
        self.background_review_active = true;
        debug!("Starting background memory review");

        let content = format!("User:\n{}\n\nAssistant:\n{}", user, assistant);

        let request = ChatRequest::new(model).with_messages(vec![
            Message::system(BACKGROUND_REVIEW_PROMPT),
            Message::user(&content),
        ]);

        match tokio::time::timeout(memory_llm_timeout(), provider.chat(request)).await {
            Ok(Ok(response)) => {
                let text = response.content.trim();
                if text.eq_ignore_ascii_case("NONE") || text.is_empty() {
                    debug!("Background review: nothing worth remembering");
                    self.background_review_active = false;
                    return;
                }
                // 复用现有的 LLM 候选解析逻辑
                let candidates = super::extraction::parse_llm_memory_candidates(
                    text,
                    self.active_scope.clone(),
                    MemoryProvenance::local("background_review"),
                );
                let count = candidates.len();
                for candidate in candidates.into_iter().take(3) {
                    self.submit_candidate_with_provider_notifications(
                        candidate,
                        MemoryWriteTarget::Auto,
                    )
                    .await;
                }
                if count > 0 {
                    debug!("Background review saved {} memory candidates", count);
                }
            }
            Ok(Err(e)) => {
                debug!("Background review LLM call failed: {}", e);
            }
            Err(_) => {
                debug!(
                    "Background review timed out after {}s",
                    memory_llm_timeout().as_secs()
                );
            }
        }

        self.background_review_active = false;
    }
}

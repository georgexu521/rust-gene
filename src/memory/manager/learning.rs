//! Memory manager learning functions.
//!
//! Functions for adding learning content to memory.

use super::helpers::log_preview;
use super::MemoryManager;
use crate::memory::files::infer_learning_topic;
use crate::memory::reports::{MemoryWriteOutcome, MemoryWriteOutcomeStatus, MemoryWriteTarget};
use tracing::debug;

impl MemoryManager {
    /// 添加学习内容（同步版本）
    pub fn add_learning(&mut self, content: &str, category: &str) {
        let candidate =
            self.candidate_from_content(content, category, "memory_manager.add_learning");
        let target = if matches!(category, "preference" | "user") {
            MemoryWriteTarget::User
        } else {
            MemoryWriteTarget::Index
        };
        let outcome = self.submit_candidate(candidate, target);
        if outcome.status == MemoryWriteOutcomeStatus::Saved {
            self.main_agent_wrote_this_turn = true;
            debug!("Memory saved: [{}] {}", category, log_preview(content, 50));
        } else {
            debug!(
                "Memory candidate not saved ({:?}): {} | {}",
                outcome.status,
                outcome.reason,
                log_preview(content, 80)
            );
        }
    }

    /// 添加学习内容到分主题记忆文件（同步版本）
    pub fn add_topic_learning(&mut self, content: &str, category: &str, topic: &str) {
        let candidate =
            self.candidate_from_content(content, category, "memory_manager.add_topic_learning");
        let outcome = self.submit_candidate(candidate, MemoryWriteTarget::Topic(topic.to_string()));
        if outcome.status == MemoryWriteOutcomeStatus::Saved {
            self.main_agent_wrote_this_turn = true;
            debug!(
                "Topic memory saved: [{}:{}] {}",
                topic,
                category,
                log_preview(content, 50)
            );
        } else {
            debug!(
                "Topic memory candidate not saved ({:?}): {} | {}",
                outcome.status,
                outcome.reason,
                log_preview(content, 80)
            );
        }
    }

    /// 自动选择 USER.md、MEMORY.md 或分主题文件保存学习内容。
    pub fn add_auto_learning(&mut self, content: &str, category: &str) {
        if matches!(category, "preference" | "user") {
            self.add_learning(content, category);
        } else if let Some(topic) = infer_learning_topic(content, category) {
            self.add_topic_learning(content, category, topic);
        } else {
            self.add_learning(content, category);
        }
    }

    /// 添加学习内容（异步版本 — 推荐在异步上下文中使用）
    pub async fn add_learning_async(&self, content: &str, category: &str) -> MemoryWriteOutcome {
        let candidate =
            self.candidate_from_content(content, category, "memory_manager.add_learning_async");
        let target = if matches!(category, "preference" | "user") {
            MemoryWriteTarget::User
        } else {
            MemoryWriteTarget::Index
        };
        self.submit_candidate_with_provider_notifications(candidate, target)
            .await
    }

    /// 添加学习内容到分主题记忆文件（异步版本）
    pub async fn add_topic_learning_async(
        &self,
        content: &str,
        category: &str,
        topic: &str,
    ) -> MemoryWriteOutcome {
        let candidate = self.candidate_from_content(
            content,
            category,
            "memory_manager.add_topic_learning_async",
        );
        self.submit_candidate_with_provider_notifications(
            candidate,
            MemoryWriteTarget::Topic(topic.to_string()),
        )
        .await
    }

    /// 自动选择 USER.md、MEMORY.md 或分主题文件保存学习内容（异步版本）。
    pub async fn add_auto_learning_async(
        &self,
        content: &str,
        category: &str,
    ) -> MemoryWriteOutcome {
        if matches!(category, "preference" | "user") {
            self.add_learning_async(content, category).await
        } else if let Some(topic) = infer_learning_topic(content, category) {
            self.add_topic_learning_async(content, category, topic)
                .await
        } else {
            self.add_learning_async(content, category).await
        }
    }
}

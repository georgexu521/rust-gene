//! Unified retrieval context contract.
//!
//! Retrieval currently comes from memory, project search, session history, web,
//! files, and MCP. This module provides one provenance-bearing shape that each
//! source can migrate to without changing prompt assembly every time.

use crate::engine::intent_router::RetrievalPolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const PREVIEW_CHARS: usize = 1200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalSource {
    Memory,
    Project,
    Session,
    Web,
    Mcp,
    File,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalItem {
    pub source: RetrievalSource,
    pub title: String,
    pub content_preview: String,
    pub score: f32,
    pub provenance: String,
    pub freshness: Option<String>,
    pub trust: TrustLevel,
    pub token_estimate: usize,
}

impl RetrievalItem {
    pub fn new(
        source: RetrievalSource,
        title: impl Into<String>,
        content: impl AsRef<str>,
        score: f32,
        provenance: impl Into<String>,
        trust: TrustLevel,
    ) -> Self {
        let content = content.as_ref();
        Self {
            source,
            title: title.into(),
            content_preview: preview(content, PREVIEW_CHARS),
            score: score.clamp(0.0, 1.0),
            provenance: provenance.into(),
            freshness: None,
            trust,
            token_estimate: estimate_tokens(content),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalContext {
    pub query: String,
    pub policy: RetrievalPolicy,
    pub created_at: DateTime<Utc>,
    pub items: Vec<RetrievalItem>,
    pub token_estimate: usize,
}

impl RetrievalContext {
    pub fn new(query: impl Into<String>, policy: RetrievalPolicy) -> Self {
        Self {
            query: query.into(),
            policy,
            created_at: Utc::now(),
            items: Vec::new(),
            token_estimate: 0,
        }
    }

    pub fn add_item(&mut self, item: RetrievalItem) {
        self.token_estimate = self.token_estimate.saturating_add(item.token_estimate);
        self.items.push(item);
        self.items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn from_memory_prefetch(
        query: &str,
        content: &str,
        policy: RetrievalPolicy,
    ) -> Option<Self> {
        if content.trim().is_empty() {
            return None;
        }
        let mut ctx = Self::new(query, policy);
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "Relevant memory",
            content,
            0.85,
            "memory.prefetch",
            TrustLevel::Medium,
        ));
        Some(ctx)
    }

    pub fn from_project_summary(
        query: &str,
        summary: &str,
        root: impl AsRef<std::path::Path>,
        policy: RetrievalPolicy,
    ) -> Option<Self> {
        if summary.trim().is_empty() {
            return None;
        }
        let mut ctx = Self::new(query, policy);
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Project,
            "Project index summary",
            summary,
            0.75,
            format!("project.index:{}", root.as_ref().display()),
            TrustLevel::High,
        ));
        Some(ctx)
    }

    pub fn item_count_by_source(&self, source: RetrievalSource) -> usize {
        self.items
            .iter()
            .filter(|item| item.source == source)
            .count()
    }

    pub fn format_for_prompt(&self) -> String {
        if self.items.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "<retrieval-context policy=\"{:?}\" tokens=\"{}\">\n",
            self.policy, self.token_estimate
        );
        for (idx, item) in self.items.iter().enumerate() {
            out.push_str(&format!(
                "<item index=\"{}\" source=\"{:?}\" score=\"{:.2}\" trust=\"{:?}\" provenance=\"{}\">\n{}\n</item>\n",
                idx + 1,
                item.source,
                item.score,
                item.trust,
                xml_escape(&item.provenance),
                item.content_preview
            ));
        }
        out.push_str("</retrieval-context>");
        out
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    // Good enough for budgeting and trace display. CJK text often maps closer
    // to one token per character, so this intentionally stays conservative.
    text.chars().count().div_ceil(4).max(1)
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        out.push(ch);
    }
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_prefetch_builds_context() {
        let ctx = RetrievalContext::from_memory_prefetch(
            "状态栏",
            "User prefers compact CLI status bars.",
            RetrievalPolicy::Memory,
        )
        .expect("context");

        assert_eq!(ctx.items.len(), 1);
        assert_eq!(ctx.item_count_by_source(RetrievalSource::Memory), 1);
        assert!(ctx.format_for_prompt().contains("<retrieval-context"));
        assert!(ctx.token_estimate > 0);
    }

    #[test]
    fn items_are_sorted_by_score() {
        let mut ctx = RetrievalContext::new("query", RetrievalPolicy::Full);
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Web,
            "low",
            "low",
            0.2,
            "web",
            TrustLevel::Low,
        ));
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Project,
            "high",
            "high",
            0.9,
            "project",
            TrustLevel::High,
        ));
        assert_eq!(ctx.items[0].title, "high");
    }

    #[test]
    fn project_summary_builds_context() {
        let ctx = RetrievalContext::from_project_summary(
            "修改 tui",
            "src/tui/mod.rs\nsrc/tui/app.rs",
            "/repo",
            RetrievalPolicy::Project,
        )
        .expect("project context");

        assert_eq!(ctx.item_count_by_source(RetrievalSource::Project), 1);
        assert!(ctx.format_for_prompt().contains("project.index:/repo"));
    }
}

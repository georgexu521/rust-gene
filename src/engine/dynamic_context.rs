//! Shared dynamic-context markers and request-tail block rendering.

use std::collections::HashSet;

pub const TASK_STATE_TAG: &str = "task-state";
pub const TASK_STATE_ALT_TAG: &str = "task_state";
pub const TASK_CONTRACT_TAG: &str = "task-contract";
pub const CONTEXT_PACK_TAG: &str = "context-pack";
pub const LAB_CONTEXT_TAG: &str = "lab-context";
pub const RELEVANT_MATERIAL_TAG: &str = "relevant_material";
pub const RECENT_OBSERVATION_TAG: &str = "recent_observation";
pub const SELF_EVOLUTION_GUIDANCE_TAG: &str = "self-evolution-guidance";
pub const TASK_GUIDANCE_TAG: &str = "task-guidance";
pub const CONTEXT_ZONES_PREFIX: &str = "<context_zones";
pub const RETRIEVAL_CONTEXT_PREFIX: &str = "<retrieval-context";
pub const MVA_PROFILE_PREFIX: &str = "MVA profile:";

pub const DYNAMIC_CONTEXT_TAGS: &[&str] = &[
    TASK_STATE_TAG,
    TASK_STATE_ALT_TAG,
    TASK_CONTRACT_TAG,
    CONTEXT_PACK_TAG,
    LAB_CONTEXT_TAG,
    RELEVANT_MATERIAL_TAG,
    RECENT_OBSERVATION_TAG,
    SELF_EVOLUTION_GUIDANCE_TAG,
    TASK_GUIDANCE_TAG,
];

pub const DYNAMIC_CONTEXT_MARKERS: &[&str] = &[
    "<task-state>",
    "<task_state>",
    "<task-contract>",
    "<context-pack>",
    "<lab-context>",
    "<relevant_material>",
    "<recent_observation>",
    "<self-evolution-guidance>",
    "<task-guidance>",
    CONTEXT_ZONES_PREFIX,
    RETRIEVAL_CONTEXT_PREFIX,
    MVA_PROFILE_PREFIX,
];

pub fn is_dynamic_context_system_message(content: &str) -> bool {
    let trimmed = content.trim_start();
    DYNAMIC_CONTEXT_MARKERS
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

pub fn user_message_contains_dynamic_context(content: &str) -> bool {
    DYNAMIC_CONTEXT_MARKERS
        .iter()
        .any(|marker| content.contains(marker))
}

#[derive(Debug, Default)]
pub struct DynamicContextBlockBuilder {
    blocks: Vec<String>,
    seen: HashSet<String>,
    duplicate_blocks_removed: usize,
}

impl DynamicContextBlockBuilder {
    pub fn push(&mut self, block: impl Into<String>) -> bool {
        let block = block.into();
        let block = block.trim();
        if block.is_empty() {
            return false;
        }
        let key = normalized_block_key(block);
        if !self.seen.insert(key) {
            self.duplicate_blocks_removed += 1;
            return false;
        }
        self.blocks.push(block.to_string());
        true
    }

    pub fn render_user_tail(&self) -> Option<String> {
        if self.blocks.is_empty() {
            return None;
        }
        let rendered = self
            .blocks
            .iter()
            .rev()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n\n");
        Some(rendered)
    }

    pub fn duplicate_blocks_removed(&self) -> usize {
        self.duplicate_blocks_removed
    }
}

pub fn tagged_block(tag: &str, body: impl AsRef<str>) -> Option<String> {
    let body = body.as_ref().trim();
    if body.is_empty() {
        return None;
    }
    Some(format!("<{tag}>\n{body}\n</{tag}>"))
}

pub fn normalized_block_key(block: &str) -> String {
    block
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
            .to_ascii_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

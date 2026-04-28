//! Prompt and context assembly for main model requests.
//!
//! Keep stable prompt layers in one place so engines do not each rebuild a
//! slightly different system prompt.

use std::path::{Path, PathBuf};

use crate::services::api::Message;

#[derive(Debug, Clone)]
pub struct PromptContextAssembler {
    base_prompt: String,
    working_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub system_prompt: String,
}

#[derive(Debug, Clone)]
pub struct PromptLayerReport {
    pub name: String,
    pub chars: usize,
    pub tokens: u64,
}

#[derive(Debug, Clone)]
pub struct PromptContextReport {
    pub layers: Vec<PromptLayerReport>,
    pub total_chars: usize,
    pub total_tokens: u64,
    pub fingerprint: String,
}

impl PromptContextAssembler {
    pub fn new(base_prompt: impl Into<String>, working_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_prompt: base_prompt.into(),
            working_dir: working_dir.into(),
        }
    }

    pub fn from_current_dir(base_prompt: impl Into<String>) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(base_prompt, working_dir)
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    pub fn build_for_turn(&self, user_message: &str, history: &[Message]) -> PromptContext {
        let layered =
            crate::instructions::compose_system_prompt(&self.base_prompt, &self.working_dir);
        let system_prompt =
            crate::engine::prompt_builder::compose_task_aware_system_prompt_with_history(
                &layered,
                user_message,
                history,
            );
        PromptContext { system_prompt }
    }

    pub fn build_for_single_user_message(&self, user_message: &str) -> PromptContext {
        let layered =
            crate::instructions::compose_system_prompt(&self.base_prompt, &self.working_dir);
        let system_prompt =
            crate::engine::prompt_builder::compose_task_aware_system_prompt(&layered, user_message);
        PromptContext { system_prompt }
    }

    pub fn report_for_turn(&self, user_message: &str, history: &[Message]) -> PromptContextReport {
        let final_prompt = self.build_for_turn(user_message, history).system_prompt;
        let stable_prompt =
            crate::instructions::compose_system_prompt(&self.base_prompt, &self.working_dir);
        let instruction_layers = crate::instructions::load_instruction_layers(&self.working_dir);

        let mut layers = Vec::new();
        layers.push(layer_report("base system prompt", &self.base_prompt));

        for layer in instruction_layers {
            layers.push(layer_report(
                format!("AGENTS.md [{}]", layer.source),
                &layer.content,
            ));
        }

        if final_prompt != stable_prompt {
            let task_focus_chars = final_prompt
                .chars()
                .count()
                .saturating_sub(stable_prompt.chars().count());
            if task_focus_chars > 0 {
                let task_focus = final_prompt
                    .chars()
                    .skip(stable_prompt.chars().count())
                    .collect::<String>();
                layers.push(layer_report("task focus", &task_focus));
            }
        }

        PromptContextReport {
            total_chars: final_prompt.chars().count(),
            total_tokens: crate::engine::context_compressor::estimate_tokens(&final_prompt),
            fingerprint: stable_fingerprint(&final_prompt),
            layers,
        }
    }
}

fn layer_report(name: impl Into<String>, content: &str) -> PromptLayerReport {
    PromptLayerReport {
        name: name.into(),
        chars: content.chars().count(),
        tokens: crate::engine::context_compressor::estimate_tokens(content),
    }
}

pub fn stable_fingerprint(content: &str) -> String {
    format!("{:x}", md5::compute(content))
        .chars()
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembler_preserves_base_prompt_for_general_turn() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);

        let prompt = assembler.build_for_turn("hello", &[]).system_prompt;

        assert!(prompt.starts_with("base prompt"));
        assert!(prompt.contains("Workspace Boundary"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assembler_adds_task_focus_with_history() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);
        let history = vec![Message::user("请做 code review")];

        let prompt = assembler.build_for_turn("继续", &history).system_prompt;

        assert!(prompt.contains("Task Focus: Code Review"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_includes_base_and_task_focus() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);

        let report = assembler.report_for_turn("请实现功能", &[]);

        assert!(report.layers.iter().any(|l| l.name == "base system prompt"));
        assert!(report.layers.iter().any(|l| l.name == "task focus"));
        assert!(report.total_tokens > 0);
        assert_eq!(report.fingerprint.len(), 12);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

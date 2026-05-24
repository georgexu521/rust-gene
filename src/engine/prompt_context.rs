//! Prompt and context assembly for main model requests.
//!
//! Keep stable prompt layers in one place so engines do not each rebuild a
//! slightly different system prompt.

use std::path::{Path, PathBuf};

use crate::engine::context_assembly::{
    ContextAssemblyInput, ContextAssemblyPlan, ContextAssemblyReport,
};
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
    pub assembly: ContextAssemblyReport,
    pub total_chars: usize,
    pub total_tokens: u64,
    pub fingerprint: String,
    pub stable_prefix_fingerprint: String,
    pub dynamic_tail_tokens: u64,
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
        let plan = self.assembly_plan_for_turn(user_message, history);
        PromptContext {
            system_prompt: plan.render_legacy_system_prompt(),
        }
    }

    pub fn build_for_single_user_message(&self, user_message: &str) -> PromptContext {
        let plan = self.assembly_plan_for_single_user_message(user_message);
        PromptContext {
            system_prompt: plan.render_legacy_system_prompt(),
        }
    }

    pub fn assembly_plan_for_turn(
        &self,
        user_message: &str,
        history: &[Message],
    ) -> ContextAssemblyPlan {
        let layered =
            crate::instructions::compose_system_prompt(&self.base_prompt, &self.working_dir);
        let system_prompt =
            crate::engine::prompt_builder::compose_task_aware_system_prompt_with_history(
                &layered,
                user_message,
                history,
            );
        context_assembly_plan_from_prompt_parts(layered, system_prompt, user_message)
    }

    pub fn assembly_plan_for_single_user_message(&self, user_message: &str) -> ContextAssemblyPlan {
        let layered =
            crate::instructions::compose_system_prompt(&self.base_prompt, &self.working_dir);
        let system_prompt =
            crate::engine::prompt_builder::compose_task_aware_system_prompt(&layered, user_message);
        context_assembly_plan_from_prompt_parts(layered, system_prompt, user_message)
    }

    pub fn report_for_turn(&self, user_message: &str, history: &[Message]) -> PromptContextReport {
        let assembly_plan = self.assembly_plan_for_turn(user_message, history);
        let final_prompt = assembly_plan.render_legacy_system_prompt();
        let stable_prompt = &assembly_plan.stable_prefix.content;
        let instruction_layers = crate::instructions::load_instruction_layers(&self.working_dir);

        let mut layers = Vec::new();
        layers.push(layer_report("base system prompt", &self.base_prompt));

        for layer in instruction_layers {
            let truncated = if layer.truncated { ",truncated" } else { "" };
            layers.push(layer_report(
                format!(
                    "AGENTS.md [{}:{}{}]",
                    layer.source,
                    layer.selection.label(),
                    truncated
                ),
                &layer.content,
            ));
        }

        if final_prompt != *stable_prompt {
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
            stable_prefix_fingerprint: assembly_plan.cache_report.stable_prefix_fingerprint.clone(),
            dynamic_tail_tokens: assembly_plan.cache_report.dynamic_tail_tokens,
            assembly: assembly_plan.report(),
            layers,
        }
    }
}

fn context_assembly_plan_from_prompt_parts(
    stable_prompt: String,
    final_prompt: String,
    user_message: &str,
) -> ContextAssemblyPlan {
    let task_state_tail = final_prompt
        .strip_prefix(&stable_prompt)
        .unwrap_or("")
        .to_string();
    ContextAssemblyPlan::new(ContextAssemblyInput {
        stable_prefix: stable_prompt,
        task_state: task_state_tail,
        relevant_material: String::new(),
        recent_observation: String::new(),
        current_decision_request: user_message.to_string(),
    })
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
        assert!(report
            .assembly
            .zones
            .iter()
            .any(|zone| zone.name == "task_state" && !zone.empty));
        assert!(report.total_tokens > 0);
        assert_eq!(report.fingerprint.len(), 12);
        assert_eq!(report.stable_prefix_fingerprint.len(), 12);
        assert!(report.dynamic_tail_tokens > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assembly_plan_reports_five_zones_in_stable_order() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);

        let plan = assembler.assembly_plan_for_turn("请实现功能", &[]);
        let zone_names = plan
            .zone_reports()
            .into_iter()
            .map(|zone| zone.name)
            .collect::<Vec<_>>();

        assert_eq!(
            zone_names,
            vec![
                "stable_prefix",
                "task_state",
                "relevant_material",
                "recent_observation",
                "current_decision_request"
            ]
        );
        assert!(!plan.stable_prefix.is_empty());
        assert!(!plan.current_decision_request.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stable_prefix_fingerprint_stays_fixed_when_task_focus_changes() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);

        let coding = assembler.report_for_turn("请实现功能", &[]);
        let review = assembler.report_for_turn("请做 code review", &[]);

        assert_eq!(
            coding.stable_prefix_fingerprint,
            review.stable_prefix_fingerprint
        );
        assert_ne!(coding.fingerprint, review.fingerprint);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_labels_runtime_guidance_instruction_layers() {
        let dir = std::env::temp_dir().join(format!("prompt-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(
            dir.join("AGENTS.md"),
            "# Project Notes\n\n## Agent Runtime Guidance\nruntime rule\n\n## Archive\nold rule",
        )
        .unwrap();
        let assembler = PromptContextAssembler::new("base prompt", &dir);

        let report = assembler.report_for_turn("hello", &[]);

        assert!(report
            .layers
            .iter()
            .any(|l| l.name == "AGENTS.md [project:runtime-guidance]"));
        assert!(!report.layers.iter().any(|l| l.name.contains("fallback")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn common_sample_prompts_stay_under_runtime_diet_prompt_budget() {
        const COMMON_TURN_PROMPT_TOKEN_BUDGET: u64 = 2_500;
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let assembler = PromptContextAssembler::new(crate::engine::default_system_prompt(), &repo);
        let samples = [
            "简单回答：2+2 等于几？",
            "帮我把这个文件删了吧",
            "帮我做一个贪吃蛇游戏吧，用 python 做吧",
            "我在运行中发现了一个问题，你帮我看看是怎么回事吧",
            "帮我对比 claude 和 opencode 的 agent 指令设计",
        ];

        for prompt in samples {
            let report = assembler.report_for_turn(prompt, &[]);
            let layer_summary = report
                .layers
                .iter()
                .map(|layer| format!("{}={}t/{}c", layer.name, layer.tokens, layer.chars))
                .collect::<Vec<_>>()
                .join(", ");

            assert!(
                report.total_tokens <= COMMON_TURN_PROMPT_TOKEN_BUDGET,
                "sample prompt exceeded runtime diet prompt budget: prompt={prompt:?}, tokens={}, budget={}, layers=[{}]",
                report.total_tokens,
                COMMON_TURN_PROMPT_TOKEN_BUDGET,
                layer_summary
            );
        }
    }
}

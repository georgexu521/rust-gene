use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::retrieval_context::RetrievalContext;
use crate::services::api::Tool;
use std::path::Path;

pub(super) struct TurnRuntimeDietBootstrapContext<'a> {
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) tools: &'a [Tool],
    pub(super) working_dir: &'a Path,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
}

pub(super) struct TurnRuntimeDietBootstrapController;

impl TurnRuntimeDietBootstrapController {
    pub(super) fn observe(context: TurnRuntimeDietBootstrapContext<'_>) {
        if let Some(retrieval_context) = context.retrieval_context {
            context
                .runtime_diet
                .observe_retrieval_context(retrieval_context);
        }
        if Self::skills_list_exposed(context.tools) {
            let skill_summary =
                crate::skills::SkillRuntime::load(context.working_dir).discovery_summary("", 30);
            context
                .runtime_diet
                .observe_skill_list_summary(&skill_summary);
        }
    }

    fn skills_list_exposed(tools: &[Tool]) -> bool {
        tools.iter().any(|tool| tool.name == "skills_list")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::RetrievalPolicy;
    use crate::engine::retrieval_context::RetrievalContext;
    use tempfile::tempdir;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[test]
    fn observe_records_retrieval_context_budget() {
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let retrieval_context = RetrievalContext::from_memory_prefetch(
            "fix bug",
            "remember to run cargo test",
            RetrievalPolicy::Memory,
        )
        .expect("memory context");
        let tmp = tempdir().expect("tempdir");

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: Some(&retrieval_context),
            tools: &[tool("file_read")],
            working_dir: tmp.path(),
            runtime_diet: &mut runtime_diet,
        });

        assert_eq!(runtime_diet.retrieval_items, 1);
        assert!(runtime_diet.retrieval_tokens > 0);
        assert_eq!(runtime_diet.skill_list_chars, 0);
    }

    #[test]
    fn observe_records_skill_summary_only_when_tool_is_exposed() {
        let tmp = tempdir().expect("tempdir");
        let mut without_skill_tool = RuntimeDietSnapshot::new(true);

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: None,
            tools: &[tool("file_read")],
            working_dir: tmp.path(),
            runtime_diet: &mut without_skill_tool,
        });

        assert_eq!(without_skill_tool.skill_list_chars, 0);

        let mut with_skill_tool = RuntimeDietSnapshot::new(true);
        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: None,
            tools: &[tool("skills_list")],
            working_dir: tmp.path(),
            runtime_diet: &mut with_skill_tool,
        });

        assert!(with_skill_tool.skill_list_chars > 0);
        assert!(with_skill_tool.skill_list_tokens > 0);
    }
}

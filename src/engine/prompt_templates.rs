//! Built-in prompt and workflow templates.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

impl PromptTemplate {
    pub fn render(&self, values: &BTreeMap<String, String>) -> String {
        let mut out = self.body.to_string();
        for (key, value) in values {
            out = out.replace(&format!("{{{{{}}}}}", key), value);
        }
        out
    }
}

pub fn builtin_templates() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate {
            name: "diagnose",
            description: "Inspect a failure before editing",
            body: "Goal: {{goal}}\n\nWorkflow:\n1. Reproduce or locate the failure.\n2. Inspect the smallest relevant code path.\n3. State the likely root cause with evidence.\n4. Make the minimal fix.\n5. Run a focused verification.\n\nAcceptance: explain what changed and report verification output.",
        },
        PromptTemplate {
            name: "implement",
            description: "Implement a scoped feature safely",
            body: "Goal: {{goal}}\n\nWorkflow:\n1. Inspect existing patterns and nearby modules.\n2. Define acceptance checks before editing.\n3. Implement the smallest coherent change.\n4. Update tests or add focused coverage when behavior changes.\n5. Run verification and summarize residual risk.",
        },
        PromptTemplate {
            name: "review",
            description: "Review code for bugs and regressions",
            body: "Goal: {{goal}}\n\nReview stance:\n- Lead with correctness, security, data loss, and regression risks.\n- Cite concrete files or behavior.\n- Separate findings from summary.\n- If no issues are found, state remaining test gaps.",
        },
        PromptTemplate {
            name: "research",
            description: "Compare external approaches and turn them into an implementation plan",
            body: "Goal: {{goal}}\n\nWorkflow:\n1. Gather current primary or authoritative sources.\n2. Extract design patterns relevant to this project.\n3. Compare against current implementation.\n4. Produce prioritized, testable next steps.",
        },
    ]
}

pub fn find_template(name: &str) -> Option<PromptTemplate> {
    builtin_templates()
        .into_iter()
        .find(|template| template.name == name)
}

pub fn list_templates() -> String {
    let mut lines = vec!["Prompt Templates:".to_string()];
    for template in builtin_templates() {
        lines.push(format!("- {:<10} {}", template.name, template.description));
    }
    lines.push("Use /prompt render <name> <goal>.".to_string());
    lines.join("\n")
}

pub fn render_template(name: &str, goal: &str) -> Result<String, String> {
    let Some(template) = find_template(name) else {
        return Err(format!(
            "Unknown prompt template '{}'. Use /prompt templates.",
            name
        ));
    };
    let mut values = BTreeMap::new();
    values.insert("goal".to_string(), goal.trim().to_string());
    Ok(template.render(&values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_builtin_templates() {
        let list = list_templates();
        assert!(list.contains("diagnose"));
        assert!(list.contains("implement"));
    }

    #[test]
    fn renders_goal_placeholder() {
        let rendered = render_template("implement", "add ResourcePolicy").unwrap();
        assert!(rendered.contains("add ResourcePolicy"));
        assert!(!rendered.contains("{{goal}}"));
    }

    #[test]
    fn rejects_unknown_template() {
        assert!(render_template("missing", "goal").is_err());
    }
}

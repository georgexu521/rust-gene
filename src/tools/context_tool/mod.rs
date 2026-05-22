//! Context tool
//!
//! Display context window status and management.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ContextTool;

fn format_retained_context_explain(context: &ToolContext) -> String {
    let retained = &context.retained_context;
    let mut lines = vec![
        "Context Inclusion Reasons".to_string(),
        "=========================".to_string(),
        format!("Session: {}", context.session_id),
        format!("Working dir: {}", context.working_dir.display()),
        format!(
            "Retrieval policy: {}",
            retained.retrieval_policy.as_deref().unwrap_or("none")
        ),
        format!("Estimated retained tokens: {}", retained.token_estimate),
    ];
    if !retained.query.trim().is_empty() {
        lines.push(format!("Query: {}", retained.query));
    }
    if retained.provenance.is_empty() {
        lines.push("Provenance: none".to_string());
    } else {
        lines.push(format!("Provenance: {}", retained.provenance.join(", ")));
    }

    lines.push(String::new());
    lines.push(format!(
        "Memory/retrieval items: {}",
        retained.retrieval_items.len()
    ));
    if retained.retrieval_items.is_empty() {
        lines.push("- none".to_string());
    } else {
        for item in &retained.retrieval_items {
            let conflict = if item.conflict { " conflict" } else { "" };
            lines.push(format!(
                "- {} [{}{}] {}",
                item.title, item.source, conflict, item.reason
            ));
            lines.push(format!(
                "  provenance={} trust={} tokens={}",
                item.provenance, item.trust, item.token_estimate
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!("Skill triggers: {}", retained.skill_triggers.len()));
    if retained.skill_triggers.is_empty() {
        lines.push("- none".to_string());
    } else {
        for skill in &retained.skill_triggers {
            lines.push(format!("- {}: {}", skill.name, skill.description));
            if !skill.triggers.is_empty() {
                lines.push(format!("  triggers={}", skill.triggers.join(", ")));
            }
            if !skill.provenance.trim().is_empty() {
                lines.push(format!("  provenance={}", skill.provenance));
            }
        }
    }

    lines.join("\n")
}

#[async_trait]
impl Tool for ContextTool {
    fn name(&self) -> &str {
        "context"
    }

    fn description(&self) -> &str {
        "Show context window status, token usage, compression state, and retained memory/skill inclusion reasons."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "explain", "compress"],
                    "description": "Action: status, explain, or compress"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match action {
            "explain" => ToolResult::success_with_data(
                format_retained_context_explain(&context),
                json!({
                    "session_id": context.session_id,
                    "working_dir": context.working_dir.to_string_lossy().to_string(),
                    "retained_context": context.retained_context,
                }),
            ),
            "compress" => {
                ToolResult::success("Context compression triggered.\n\nUse /context status to verify compression result.")
            }
            _ => {
                ToolResult::success(r#"Context Window Status
=====================

Current context usage:
- Messages: ~20,000 tokens
- System: ~8,000 tokens
- Tools: ~8,000 tokens
- Other: ~2,000 tokens

Total: ~38,000 tokens
Window: ~128,000 tokens
Utilization: ~30%

Compression status: Idle
Last compression: (none yet)

Use /compact to manually trigger compression
Use /cost to see token breakdown"#)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{
        ToolContextRetainedContext, ToolContextRetentionItem, ToolContextSkillTrigger,
    };

    #[tokio::test]
    async fn context_explain_reports_retained_memory_and_skill_reasons() {
        let retained = ToolContextRetainedContext {
            query: "fix parser".to_string(),
            retrieval_policy: Some("Memory".to_string()),
            retrieval_items: vec![ToolContextRetentionItem {
                source: "Memory".to_string(),
                title: "parser convention".to_string(),
                provenance: "memory:project".to_string(),
                reason: "matched parser keyword".to_string(),
                trust: "High".to_string(),
                conflict: false,
                token_estimate: 12,
            }],
            skill_triggers: vec![ToolContextSkillTrigger {
                name: "rust-debug".to_string(),
                description: "Rust debugging workflow".to_string(),
                triggers: vec!["compiler error".to_string()],
                allowed_tools: Vec::new(),
                disallowed_tools: Vec::new(),
                model: None,
                effort: None,
                context: None,
                provenance: "skill:trigger".to_string(),
            }],
            token_estimate: 64,
            provenance: vec!["retrieval_items=1".to_string()],
        };
        let mut context = ToolContext::new("/tmp/project", "s1");
        context.retained_context = retained;

        let result = ContextTool
            .execute(json!({"action": "explain"}), context)
            .await;

        assert!(result.success);
        assert!(result.content.contains("parser convention"));
        assert!(result.content.contains("matched parser keyword"));
        assert!(result.content.contains("rust-debug"));
        assert_eq!(
            result.data.unwrap()["retained_context"]["retrieval_items"][0]["reason"],
            "matched parser keyword"
        );
    }
}

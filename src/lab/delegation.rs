//! LabRun support module.
//!
//! Keeps LabRun scheduling, delegation, reporting, and certification helpers separate from normal agent turns.

use crate::agent::envelope::AgentTaskEnvelope;
use crate::agent::types::AgentId;
use crate::lab::model::GraduateTask;
use crate::tools::agent_tool::AgentTool;
use crate::tools::{Tool, ToolContext, ToolResult};
use anyhow::anyhow;
use serde_json::{json, Value};

const LAB_GRADUATE_ALLOWED_TOOLS: &[&str] = &[
    "grep",
    "file_read",
    "file_edit",
    "file_write",
    "bash",
    "diff",
    "format",
];

#[derive(Debug, Clone)]
pub struct GraduateTaskDispatch {
    pub envelope: AgentTaskEnvelope,
    pub agent_tool_params: Value,
}

pub fn graduate_agent_task_id(task: &GraduateTask) -> String {
    format!("lab-graduate-{}", task.task_id)
}

pub fn build_graduate_task_dispatch(task: &GraduateTask) -> anyhow::Result<GraduateTaskDispatch> {
    validate_graduate_task_for_dispatch(task)?;

    let prompt = graduate_task_prompt(task);
    let agent_task_id = graduate_agent_task_id(task);
    let mut envelope = AgentTaskEnvelope::new(
        AgentId(format!("lab-postdoc:{}", task.lab_run_id)),
        task.title.clone(),
        prompt,
    )
    .assign_to(AgentId(format!("lab-graduate:{}", task.task_id)));
    envelope.parent_task_id = Some(task.task_id.clone());
    envelope.add_context_ref(format!("labrun:{}", task.lab_run_id));
    envelope.add_context_ref(format!("graduate_task:{}", task.task_id));
    for scope in &task.allowed_scope {
        envelope.add_context_ref(scope.clone());
        envelope.add_constraint(format!("allowed_scope: {scope}"));
    }
    for command in &task.required_validation {
        envelope.add_constraint(format!("required_validation: {command}"));
    }
    for evidence_id in &task.evidence_ids {
        envelope.add_context_ref(format!("evidence:{}", evidence_id));
    }
    envelope.add_constraint(format!("agent_task_id: {agent_task_id}"));
    envelope.add_expected_artifact("GraduateResult");
    envelope.add_expected_artifact("changed_files");
    envelope.add_expected_artifact("validation_results");
    envelope.add_expected_artifact("blockers");

    let agent_tool_params = json!({
        "task_id": agent_task_id,
        "description": task.title,
        "prompt": envelope.prompt,
        "files": task.allowed_scope,
        "profile": "lab-graduate",
        "context_mode": "isolated_worktree_fork",
        "allowed_tools": LAB_GRADUATE_ALLOWED_TOOLS,
        "timeout_secs": 420,
        "max_turns": 6,
        "background": false,
    });

    Ok(GraduateTaskDispatch {
        envelope,
        agent_tool_params,
    })
}

pub async fn execute_graduate_task_with_agent_tool(
    task: &GraduateTask,
    context: ToolContext,
) -> ToolResult {
    let dispatch = match build_graduate_task_dispatch(task) {
        Ok(dispatch) => dispatch,
        Err(err) => return ToolResult::error(format!("Invalid graduate task dispatch: {err}")),
    };
    let tool = AgentTool::with_working_dir(&context.working_dir);
    tool.execute(dispatch.agent_tool_params, context).await
}

fn validate_graduate_task_for_dispatch(task: &GraduateTask) -> anyhow::Result<()> {
    if task.task_id.trim().is_empty() {
        return Err(anyhow!("graduate task_id cannot be empty"));
    }
    if task.title.trim().is_empty() {
        return Err(anyhow!("graduate task title cannot be empty"));
    }
    if task.instructions.trim().is_empty() {
        return Err(anyhow!("graduate task instructions cannot be empty"));
    }
    if task.allowed_scope.is_empty() {
        return Err(anyhow!(
            "graduate task {} cannot dispatch without allowed_scope",
            task.task_id
        ));
    }
    if task.required_validation.is_empty() {
        return Err(anyhow!(
            "graduate task {} cannot dispatch without required_validation",
            task.task_id
        ));
    }
    Ok(())
}

fn graduate_task_prompt(task: &GraduateTask) -> String {
    format!(
        "LabRun graduate task\n\
         \n\
         lab_run_id: {lab_run_id}\n\
         task_id: {task_id}\n\
         title: {title}\n\
         cycle_id: {cycle_id}\n\
         \n\
         Instructions:\n{instructions}\n\
         \n\
         Allowed scope:\n{allowed_scope}\n\
         \n\
         Required validation:\n{required_validation}\n\
         \n\
         Hard rules:\n\
         - Work only inside the allowed scope above.\n\
         - Use the provided tools for file changes and validation commands; do not write XML-like pseudo tool tags such as <bash> or <file_edit> in normal text.\n\
         - If the task asks you to create or edit a file, call file_write or file_edit before your final JSON.\n\
         - If required validation is listed, call bash with the validation command before your final JSON.\n\
         - Run the required validation when possible.\n\
         - If you cannot call the required tools, return a blocker instead of claiming completion.\n\
         - If scope or validation is insufficient, report a blocker instead of expanding the task.\n\
         - Return changed files, validation attempts, blockers, and handoff notes.\n\
         - Your result is not proof by itself; the postdoc/runtime must verify it.\n\
         \n\
         Final output contract:\n\
         Return only a JSON object as the final answer, with no Markdown fence or extra prose, using this shape:\n\
         {{\n\
           \"graduate_result\": {{\n\
             \"summary\": \"what changed and why\",\n\
             \"changed_files\": [\"relative/path.rs\"],\n\
             \"validation_results\": [\"command and result\"],\n\
             \"blockers\": [],\n\
             \"evidence_ids\": []\n\
           }}\n\
         }}",
        lab_run_id = task.lab_run_id,
        task_id = task.task_id,
        title = task.title,
        cycle_id = task.cycle_id.as_deref().unwrap_or("none"),
        instructions = task.instructions.trim(),
        allowed_scope = bullet_list(&task.allowed_scope),
        required_validation = bullet_list(&task.required_validation),
    )
}

fn bullet_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {}", value.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{GraduateTask, LabRole, LabTaskStatus, LAB_SCHEMA_VERSION};
    use chrono::Utc;

    fn task() -> GraduateTask {
        let now = Utc::now();
        GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: "gradtask_test".to_string(),
            lab_run_id: "labrun_test".to_string(),
            created_at: now,
            updated_at: now,
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::Queued,
            title: "Implement scoped slice".to_string(),
            instructions: "Update the lab model and store tests.".to_string(),
            allowed_scope: vec!["src/lab/model.rs".to_string()],
            required_validation: vec!["cargo check -q".to_string()],
            evidence_ids: vec!["labevidence_001".to_string()],
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some("0".to_string()),
        }
    }

    #[test]
    fn graduate_task_dispatch_preserves_scope_validation_and_profile() {
        let dispatch = build_graduate_task_dispatch(&task()).unwrap();

        assert_eq!(
            dispatch.envelope.parent_task_id.as_deref(),
            Some("gradtask_test")
        );
        assert!(dispatch
            .envelope
            .constraints
            .contains(&"allowed_scope: src/lab/model.rs".to_string()));
        assert!(dispatch
            .envelope
            .constraints
            .contains(&"required_validation: cargo check -q".to_string()));
        assert!(dispatch
            .envelope
            .constraints
            .contains(&"agent_task_id: lab-graduate-gradtask_test".to_string()));
        assert_eq!(
            dispatch.agent_tool_params["task_id"].as_str(),
            Some("lab-graduate-gradtask_test")
        );
        assert_eq!(
            dispatch.agent_tool_params["background"].as_bool(),
            Some(false)
        );
        assert_eq!(
            dispatch.agent_tool_params["profile"].as_str(),
            Some("lab-graduate")
        );
        assert!(dispatch.agent_tool_params["allowed_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool.as_str() == Some("file_write")));
        assert_eq!(
            dispatch.agent_tool_params["context_mode"].as_str(),
            Some("isolated_worktree_fork")
        );
        assert!(dispatch.agent_tool_params["prompt"]
            .as_str()
            .unwrap()
            .contains("Your result is not proof by itself"));
        assert!(dispatch.agent_tool_params["prompt"]
            .as_str()
            .unwrap()
            .contains("pseudo tool tags"));
        assert!(dispatch.agent_tool_params["prompt"]
            .as_str()
            .unwrap()
            .contains("\"graduate_result\""));
    }

    #[test]
    fn graduate_task_dispatch_requires_scope_and_validation() {
        let mut task = task();
        task.allowed_scope.clear();
        assert!(build_graduate_task_dispatch(&task)
            .unwrap_err()
            .to_string()
            .contains("allowed_scope"));

        task.allowed_scope.push("src/lab/model.rs".to_string());
        task.required_validation.clear();
        assert!(build_graduate_task_dispatch(&task)
            .unwrap_err()
            .to_string()
            .contains("required_validation"));
    }

    #[tokio::test]
    async fn graduate_task_executor_uses_existing_agent_tool_availability() {
        let result =
            execute_graduate_task_with_agent_tool(&task(), ToolContext::new(".", "lab-test")).await;

        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("AgentManager not available"));
    }
}

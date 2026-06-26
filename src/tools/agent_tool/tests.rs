use super::*;
use std::sync::Arc;
use std::time::Instant;

fn completed_agent_result(agent_id: &str) -> ManagerAgentResult {
    ManagerAgentResult {
        agent_id: AgentId(agent_id.to_string()),
        status: AgentStatus::Completed,
        content: "checked the implementation".to_string(),
        completed_at: Instant::now(),
        tools_used: vec!["bash".to_string()],
        confidence: 0.8,
        has_conflict: false,
    }
}

#[test]
fn agent_tool_contract_discourages_blocking_delegation() {
    let tool = AgentTool::new();
    assert!(tool.description().contains("concurrently"));
    assert!(tool.description().contains("When NOT to use"));
    assert!(tool.description().contains("profile parameter"));
    assert!(!tool
        .description()
        .contains("role parameter to select which agent type"));
    assert!(tool.description().contains("explorer"));
    assert!(tool.description().contains("verifier"));
    assert!(tool.description().contains("planner"));
    assert!(tool.description().contains("implementer"));
    assert!(
        tool.parameters()["properties"]["allowed_tools"]["description"]
            .as_str()
            .unwrap_or("")
            .contains("narrow tasks")
    );
    assert!(
        tool.parameters()["properties"]["context_mode"]["description"]
            .as_str()
            .unwrap_or("")
            .contains("isolated_worktree_fork")
    );
    assert!(tool.parameters()["properties"]["background"]["description"]
        .as_str()
        .unwrap_or("")
        .contains("completion sink"));
}

#[test]
fn default_subagent_tool_surfaces_are_role_scoped() {
    let explorer = default_subagent_allowed_tools(AgentRole::Default, Some(AgentTemplate::Explore));
    assert!(explorer.contains(&"file_read".to_string()));
    assert!(!explorer.contains(&"file_edit".to_string()));
    assert!(!explorer.contains(&"file_write".to_string()));
    assert!(!explorer.contains(&"agent".to_string()));

    let verifier =
        default_subagent_allowed_tools(AgentRole::Verification, Some(AgentTemplate::Verify));
    assert!(verifier.contains(&"bash".to_string()));
    assert!(!verifier.contains(&"file_edit".to_string()));
    assert!(!verifier.contains(&"file_write".to_string()));

    let implementer = default_subagent_allowed_tools(AgentRole::Specialist, None);
    assert!(implementer.contains(&"file_edit".to_string()));
    assert!(implementer.contains(&"file_write".to_string()));
    assert!(!implementer.contains(&"agent".to_string()));
    assert!(!implementer.contains(&"swarm".to_string()));

    let planner = default_subagent_allowed_tools(AgentRole::Plan, Some(AgentTemplate::Plan));
    assert!(planner.contains(&"plan".to_string()));
    assert!(planner.contains(&"todo_write".to_string()));
    assert!(!planner.contains(&"bash".to_string()));
}

#[tokio::test]
async fn lab_graduate_profile_requires_execution_binding_before_spawn() {
    let tool = AgentTool::new();
    let context = crate::tools::ToolContext::new(".", "agent-tool-lab-binding")
        .with_agent_manager(Arc::new(crate::agent::AgentManager::new()));

    let result = tool
        .execute(
            json!({
                "description": "Try graduate work",
                "prompt": "Change README.md",
                "profile": "lab-graduate",
                "allowed_tools": ["file_write"],
            }),
            context,
        )
        .await;

    assert!(!result.success);
    let error = result.error.as_deref().unwrap_or_default();
    assert!(
        error.contains("requires a valid LabExecutionBinding"),
        "unexpected agent tool error: {error}"
    );
}

#[test]
fn subagent_proof_metadata_marks_child_output_as_claim_only() {
    let result = completed_agent_result("agent_1");
    let allowed_tools = vec![
        "grep".to_string(),
        "file_read".to_string(),
        "bash".to_string(),
    ];
    let mut data = json!({
        "agent_id": result.agent_id.to_string(),
        "status": "completed",
        "result": result.content.clone(),
    });

    attach_subagent_proof_metadata(
        &mut data,
        &result,
        AgentRole::Verification,
        Some(AgentTemplate::Verify),
        &allowed_tools,
        false,
    );

    assert_eq!(data["proof_kind"], "subagent_claim_only");
    assert_eq!(data["verification_proof_kind"], "subagent_claim_only");
    assert_eq!(data["parent_verified"], false);
    assert_eq!(data["source_agent"], "agent_1");
    assert_eq!(data["subagent_output_kind"], "SubagentVerificationClaim");
    assert_eq!(
        data["claim_id"],
        "subagent:agent_1:SubagentVerificationClaim"
    );
    assert_eq!(data["claim_type"], "SubagentVerificationClaim");
    assert_eq!(data["related_to_changed_files"], "none");
}

#[test]
fn mutating_subagent_metadata_is_patch_summary_but_still_claim_only() {
    let result = completed_agent_result("agent_2");
    let allowed_tools = vec!["file_edit".to_string(), "bash".to_string()];
    let mut data = json!({"agent_id": "agent_2", "status": "completed"});

    attach_subagent_proof_metadata(
        &mut data,
        &result,
        AgentRole::Specialist,
        Some(AgentTemplate::Debug),
        &allowed_tools,
        false,
    );

    assert_eq!(data["subagent_output_kind"], "SubagentPatchSummary");
    assert_eq!(data["verification_proof_kind"], "subagent_claim_only");
    assert_eq!(data["claim_id"], "subagent:agent_2:SubagentPatchSummary");
    assert_eq!(data["claim_type"], "SubagentPatchSummary");
    assert_eq!(data["related_to_changed_files"], "unknown_child_worktree");
}

#[test]
fn synthesized_subtask_results_include_subagent_proof_metadata() {
    let result = completed_agent_result("agent_3");
    let allowed_tools = vec!["bash".to_string()];

    let tool_result = synthesize_results(
        "verify change",
        vec![result],
        &[],
        AgentRole::Verification,
        Some(AgentTemplate::Verify),
        &allowed_tools,
    );
    let data = tool_result.data.expect("subtask result data");

    assert_eq!(data["verification_proof_kind"], "subagent_claim_only");
    assert_eq!(data["scope"], "subagent_result_set");
    assert_eq!(
        data["results"][0]["subagent_output_kind"],
        "SubagentVerificationClaim"
    );
    assert_eq!(
        data["results"][0]["verification_proof_kind"],
        "subagent_claim_only"
    );
}

#[test]
fn mutating_tool_surface_defaults_to_isolated_worktree_context() {
    let tools = vec!["file_read".to_string(), "file_edit".to_string()];

    let mode = effective_agent_context_mode(None, None, &tools);

    assert_eq!(mode, Some(AgentContextMode::IsolatedWorktreeFork));
}

#[test]
fn read_only_tool_surface_does_not_force_worktree_context() {
    let tools = vec!["grep".to_string(), "file_read".to_string()];

    let mode = effective_agent_context_mode(None, None, &tools);

    assert_eq!(mode, None);
}

#[test]
fn explicit_context_mode_overrides_mutating_tool_inference() {
    let tools = vec!["file_write".to_string()];

    let mode = effective_agent_context_mode(Some(AgentContextMode::FullFork), None, &tools);

    assert_eq!(mode, Some(AgentContextMode::FullFork));
}

#[test]
fn agent_wait_failure_status_distinguishes_timeout() {
    let timeout = anyhow::anyhow!("Timeout waiting for agent abc result after 1s");
    let closed = anyhow::anyhow!("Agent abc result channel closed without result");

    assert_eq!(agent_wait_failure_status(&timeout), "timed_out");
    assert_eq!(agent_wait_failure_status(&closed), "failed");
}

#[test]
fn durable_subagent_task_id_sanitizes_user_supplied_ids() {
    assert_eq!(
        durable_subagent_task_id(Some(" lab task / 1 ")),
        "lab-task-1"
    );
    assert!(durable_subagent_task_id(None).starts_with("task-"));
}

#[test]
fn cancelled_agent_task_state_preserves_cleanup_metadata() {
    let state = crate::session_store::AgentTaskStateRecord {
        id: 1,
        session_id: "s1".to_string(),
        task_id: "task_1".to_string(),
        agent_id: "agent_1".to_string(),
        profile: Some("implementer".to_string()),
        role: "specialist".to_string(),
        status: "running".to_string(),
        description: "edit code".to_string(),
        transcript_path: Some("/tmp/a2a.jsonl".to_string()),
        tool_ids_in_progress: vec!["tool_1".to_string()],
        permission_requests: vec!["file_write".to_string()],
        result_artifact_id: Some(9),
        cleanup_hooks: vec!["worktree_cleanup".to_string()],
        payload: json!({
            "isolated_worktree": {
                "path": "/tmp/agent-worktree",
                "branch": "codex/agent-1234"
            }
        }),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    };

    let upsert = cancelled_agent_task_state_upsert(&state);

    assert_eq!(upsert.status, "cancelled");
    assert!(upsert.tool_ids_in_progress.is_empty());
    assert_eq!(upsert.cleanup_hooks, vec!["worktree_cleanup"]);
    assert_eq!(
        upsert.payload["isolated_worktree"]["branch"].as_str(),
        Some("codex/agent-1234")
    );
    assert!(upsert.payload["cancelled_at"].as_str().is_some());
}

#[test]
fn ensure_child_session_creates_parent_linked_session() {
    let store = crate::session_store::SessionStore::in_memory().unwrap();
    store
        .create_session("parent", "parent", "test-model", Some("/repo"))
        .unwrap();

    ensure_child_session(
        &store,
        "parent:subagent:task-1",
        "edit code",
        "parent",
        "test-model",
        Some("/repo"),
    );

    let child = store
        .get_session("parent:subagent:task-1")
        .unwrap()
        .unwrap();
    assert_eq!(child.parent_session_id.as_deref(), Some("parent"));
    assert_eq!(child.workspace_root.as_deref(), Some("/repo"));
}

#[test]
fn agent_tool_schema_exposes_lifecycle_actions() {
    let tool = AgentTool::new();
    let params = tool.parameters();

    assert_eq!(
        params["properties"]["action"]["enum"]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["list", "resume", "read", "cancel"])
    );
    assert!(!tool.requires_confirmation(&json!({"action": "list"})));
    assert!(tool
        .confirmation_prompt(&json!({"agent_id": "agent_1", "action": "cancel"}))
        .unwrap()
        .contains("Cancel running sub-agent agent_1"));
    assert!(tool
        .confirmation_prompt(&json!({"agent_id": "agent_1", "action": "read"}))
        .unwrap()
        .contains("Read durable state for sub-agent agent_1"));
}

#[tokio::test]
async fn agent_list_reads_durable_progress_without_manager() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("s1", "agent list test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "running".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: vec!["tool_1".to_string()],
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: json!({}),
        })
        .unwrap();

    let result = AgentTool::new()
        .execute(
            json!({"action": "list"}),
            ToolContext::new(".", "s1").with_session_store(store),
        )
        .await;

    assert!(result.success, "list failed: {:?}", result.error);
    assert!(result.content.contains("Sub-agent progress"));
    assert!(result
        .content
        .contains("agent_1 / task_1 [running] edit code"));
    assert_eq!(
        result.data.unwrap()["durable_tasks"][0]["agent_id"],
        "agent_1"
    );
}

#[tokio::test]
async fn agent_read_does_not_require_manager() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("s1", "agent read test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "completed".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: json!({}),
        })
        .unwrap();

    let result = AgentTool::new()
        .execute(
            json!({"agent_id": "agent_1", "action": "read"}),
            ToolContext::new(".", "s1").with_session_store(store),
        )
        .await;

    assert!(result.success, "read failed: {:?}", result.error);
    assert!(result.content.contains("Sub-agent agent_1"));
    assert_eq!(result.data.unwrap()["status"], "completed");
}

#[tokio::test]
async fn agent_resume_by_task_id_reads_durable_state_without_manager() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("s1", "agent resume test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "completed".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: Vec::new(),
            payload: json!({}),
        })
        .unwrap();

    let result = AgentTool::new()
        .execute(
            json!({"task_id": "task_1", "action": "resume"}),
            ToolContext::new(".", "s1").with_session_store(store),
        )
        .await;

    assert!(result.success, "resume failed: {:?}", result.error);
    assert_eq!(result.data.unwrap()["task_id"], "task_1");
}

#[tokio::test]
async fn agent_resume_by_agent_id_reads_durable_state_without_manager() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("s1", "agent resume test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "completed".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: Vec::new(),
            payload: json!({}),
        })
        .unwrap();

    let result = AgentTool::new()
        .execute(
            json!({"agent_id": "agent_1", "action": "resume"}),
            ToolContext::new(".", "s1").with_session_store(store),
        )
        .await;

    assert!(result.success, "resume failed: {:?}", result.error);
    assert_eq!(result.data.unwrap()["agent_id"], "agent_1");
}

#[tokio::test]
async fn agent_cancel_by_task_id_marks_durable_state_without_manager() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("s1", "agent cancel test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "running".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: vec!["tool_1".to_string()],
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: json!({}),
        })
        .unwrap();

    let result = AgentTool::new()
        .execute(
            json!({"task_id": "task_1", "action": "cancel"}),
            ToolContext::new(".", "s1").with_session_store(store.clone()),
        )
        .await;

    assert!(result.success, "cancel failed: {:?}", result.error);
    let state = store.agent_task_state("s1", "task_1").unwrap().unwrap();
    assert_eq!(state.status, "cancelled");
    assert_eq!(state.cleanup_hooks, vec!["worktree_cleanup"]);
}

#[test]
fn durable_agent_read_formats_state_and_artifact() {
    let state = crate::session_store::AgentTaskStateRecord {
        id: 1,
        session_id: "s1".to_string(),
        task_id: "task_1".to_string(),
        agent_id: "agent_1".to_string(),
        profile: Some("implementer".to_string()),
        role: "specialist".to_string(),
        status: "completed".to_string(),
        description: "edit code".to_string(),
        transcript_path: Some("/tmp/a2a.jsonl".to_string()),
        tool_ids_in_progress: Vec::new(),
        permission_requests: vec!["file_write".to_string()],
        result_artifact_id: Some(9),
        cleanup_hooks: vec!["worktree_cleanup".to_string()],
        payload: json!({}),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    };
    let artifact = crate::session_store::AgentArtifactRecord {
        id: 9,
        session_id: "s1".to_string(),
        agent_id: "agent_1".to_string(),
        profile: Some("implementer".to_string()),
        role: "specialist".to_string(),
        status: "completed".to_string(),
        description: "edit code".to_string(),
        output: "changed src/lib.rs".to_string(),
        payload: json!({ "confidence": 0.8 }),
        created_at: "now".to_string(),
    };

    let rendered = format_durable_agent_read(&state, Some(&artifact));

    assert!(rendered.contains("Sub-agent agent_1"));
    assert!(rendered.contains("Status: completed"));
    assert!(rendered.contains("Cleanup: worktree_cleanup"));
    assert!(rendered.contains("Result artifact 9 [completed]:"));
    assert!(rendered.contains("changed src/lib.rs"));
}

#[test]
fn resolved_subagent_tools_apply_definition_scope() {
    let mut profile = crate::agent::profiles::find_profile(".", "default").unwrap();
    profile.allowed_tools = vec!["file_read".to_string(), "agent".to_string()];
    profile.disallowed_tools = vec!["agent".to_string()];
    profile.mcp_servers = vec!["github".to_string()];
    let definition = AgentDefinition::from_profile(&profile);

    let tools = resolve_subagent_allowed_tools(
        Vec::new(),
        Some(&profile),
        Some(&definition),
        profile.role,
        None,
    );

    assert!(tools.contains(&"file_read".to_string()));
    assert!(tools.contains(&"mcp_tool".to_string()));
    assert!(tools.contains(&"list_mcp_resources".to_string()));
    assert!(tools.contains(&"read_mcp_resource".to_string()));
    assert!(!tools.contains(&"agent".to_string()));
}

#[tokio::test]
async fn test_agent_tool_without_manager() {
    let tool = AgentTool::new();
    let ctx = ToolContext::new(".", "test");
    let result = tool
        .execute(
            json!({
                "description": "test",
                "prompt": "do something"
            }),
            ctx,
        )
        .await;
    assert!(!result.success);
    assert!(result
        .error
        .unwrap_or_default()
        .contains("AgentManager not available"));
}

#[tokio::test]
async fn test_agent_tool_validation() {
    let tool = AgentTool::new();
    let ctx = ToolContext::new(".", "test");

    // Empty description
    let result = tool
        .execute(
            json!({
                "description": "",
                "prompt": "do something"
            }),
            ctx.clone(),
        )
        .await;
    assert!(!result.success);

    // Empty prompt
    let result = tool
        .execute(
            json!({
                "description": "test",
                "prompt": ""
            }),
            ctx,
        )
        .await;
    assert!(!result.success);
}

#[tokio::test]
async fn test_agent_tool_resume_not_found() {
    let tool = AgentTool::new();
    let ctx = ToolContext::new(".", "test");
    let result = tool
        .execute(
            json!({
                "agent_id": "nonexistent-agent-id"
            }),
            ctx,
        )
        .await;
    assert!(!result.success);
}

#[tokio::test]
async fn test_agent_tool_subtasks_validation() {
    let tool = AgentTool::new();
    let ctx = ToolContext::new(".", "test");

    // Empty subtasks
    let result = tool
        .execute(
            json!({
                "subtasks": []
            }),
            ctx.clone(),
        )
        .await;
    assert!(!result.success);

    // Missing prompt in subtask
    let result = tool
        .execute(
            json!({
                "subtasks": [{"description": "task1"}]
            }),
            ctx,
        )
        .await;
    assert!(!result.success);
}

#[test]
fn test_agent_templates() {
    assert!(AgentTemplate::from_str("explore").is_some());
    assert!(AgentTemplate::from_str("verify").is_some());
    assert!(AgentTemplate::from_str("plan").is_some());
    assert!(AgentTemplate::from_str("general").is_some());
    assert!(AgentTemplate::from_str("review").is_some());
    assert!(AgentTemplate::from_str("debug").is_some());
    assert!(AgentTemplate::from_str("unknown").is_none());
}

#[test]
fn isolated_worktree_slug_is_stable_and_safe() {
    assert_eq!(
        isolated_worktree_slug("Edit src/agent profiles.rs now"),
        "edit-src-agent-profiles-rs-now"
    );
    assert_eq!(isolated_worktree_slug("///"), "worker");
    assert!(
        isolated_worktree_slug("A very long isolated worker description that should be capped")
            .len()
            <= 32
    );
}

#[tokio::test]
async fn test_load_file_context() {
    let tmp = std::env::temp_dir().join("agent-tool-test");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("test.txt"), "hello world").unwrap();

    let ctx = load_file_context(&["test.txt".to_string()], &tmp).await;
    assert!(ctx.contains("hello world"));
    assert!(ctx.contains("## File: test.txt"));

    let _ = std::fs::remove_dir_all(tmp);
}

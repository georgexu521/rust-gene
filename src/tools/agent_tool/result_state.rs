//! Agent tool support module.
//!
//! Holds result-state and support helpers for delegated agent execution without widening the public tool contract.

use super::*;
use crate::agent::manager::AgentCompletionSink;

pub(super) fn tools_allow_file_mutation(tools: &[String]) -> bool {
    tools
        .iter()
        .any(|tool| matches!(tool.as_str(), "file_edit" | "file_write" | "apply_patch"))
}

pub(super) fn effective_agent_context_mode(
    override_mode: Option<AgentContextMode>,
    definition: Option<&AgentDefinition>,
    allowed_tools: &[String],
) -> Option<AgentContextMode> {
    override_mode
        .or_else(|| definition.map(|definition| definition.context_mode))
        .or_else(|| {
            if tools_allow_file_mutation(allowed_tools) {
                Some(AgentContextMode::IsolatedWorktreeFork)
            } else {
                None
            }
        })
}

pub(super) fn agent_wait_failure_status(error: &anyhow::Error) -> &'static str {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timeout") || message.contains("timed out") {
        "timed_out"
    } else {
        "failed"
    }
}

pub(super) fn durable_subagent_task_id(requested: Option<&str>) -> String {
    let raw = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("task-{}", uuid::Uuid::new_v4().simple()));
    let mut sanitized = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        format!("task-{}", uuid::Uuid::new_v4().simple())
    } else {
        sanitized.chars().take(96).collect()
    }
}

fn child_session_id(parent_session_id: &str, task_id: &str) -> String {
    format!("{}:subagent:{}", parent_session_id, task_id)
}

pub(super) fn ensure_child_session(
    store: &crate::session_store::SessionStore,
    child_session_id: &str,
    description: &str,
    parent_session_id: &str,
    model: &str,
    workspace_root: Option<&str>,
) {
    match store.get_session(child_session_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            let title = format!("Sub-agent: {}", description);
            if let Err(err) = store.create_child_session(
                child_session_id,
                &title,
                model,
                parent_session_id,
                workspace_root,
            ) {
                tracing::warn!(
                    "Failed to create durable sub-agent child session {}: {}",
                    child_session_id,
                    err
                );
            }
        }
        Err(err) => {
            tracing::warn!(
                "Failed to inspect durable sub-agent child session {}: {}",
                child_session_id,
                err
            );
        }
    }
}

pub(super) fn subagent_output_kind(
    status: AgentStatus,
    role: AgentRole,
    template: Option<AgentTemplate>,
    allowed_tools: &[String],
) -> &'static str {
    if status != AgentStatus::Completed {
        return "SubagentBlocked";
    }
    if tools_allow_file_mutation(allowed_tools) {
        return "SubagentPatchSummary";
    }
    if matches!(
        template,
        Some(AgentTemplate::Verify | AgentTemplate::CodeReview)
    ) || role == AgentRole::Verification
    {
        return "SubagentVerificationClaim";
    }
    "SubagentFinding"
}

pub(super) fn attach_subagent_proof_metadata(
    data: &mut serde_json::Value,
    result: &ManagerAgentResult,
    role: AgentRole,
    template: Option<AgentTemplate>,
    allowed_tools: &[String],
    parent_verified: bool,
) {
    let proof_kind = if parent_verified {
        PARENT_VERIFIED_SUBAGENT_PROOF_KIND
    } else {
        SUBAGENT_CLAIM_PROOF_KIND
    };
    let output_kind = subagent_output_kind(result.status, role, template, allowed_tools);
    let claim_id = format!("subagent:{}:{}", result.agent_id, output_kind);
    let related_to_changed_files = if tools_allow_file_mutation(allowed_tools) {
        "unknown_child_worktree"
    } else {
        "none"
    };
    data["proof_kind"] = json!(proof_kind);
    data["verification_proof_kind"] = json!(proof_kind);
    data["source_agent"] = json!(result.agent_id.to_string());
    data["tools_used"] = json!(result.tools_used);
    data["parent_verified"] = json!(parent_verified);
    data["subagent_output_kind"] = json!(output_kind);
    data["claim_id"] = json!(claim_id);
    data["claim_type"] = json!(output_kind);
    data["scope"] = json!("subagent_result");
    data["related_to_changed_files"] = json!(related_to_changed_files);
    data["residual_risk"] = json!(if parent_verified {
        "parent runtime verified subagent result"
    } else {
        "subagent output is a claim until parent runtime verification"
    });
}

/// 创建并等待单个子 Agent 完成
#[allow(clippy::too_many_arguments)]
pub(super) async fn spawn_single_agent(
    agent_manager: &crate::agent::AgentManager,
    description: &str,
    prompt: &str,
    files: &[String],
    requested_task_id: Option<&str>,
    wait_for_completion: bool,
    timeout_secs: u64,
    max_turns: usize,
    max_cost_usd: Option<f64>,
    allowed_tools: &[String],
    role: AgentRole,
    template: Option<AgentTemplate>,
    definition: Option<&AgentDefinition>,
    context_mode_override: Option<AgentContextMode>,
    force_fork_context: bool,
    context: &ToolContext,
) -> anyhow::Result<ManagerAgentResult> {
    let started_at = std::time::Instant::now();
    let task_id = durable_subagent_task_id(requested_task_id);
    let child_session_id = child_session_id(&context.session_id, &task_id);
    let subagent_session_policy =
        derive_subagent_session_policy(&context.permission_context, definition, allowed_tools);
    let exposed_tools = subagent_session_policy.exposed_tools.clone();
    let effective_context_mode =
        effective_agent_context_mode(context_mode_override, definition, &exposed_tools);
    let isolated_worktree = if effective_context_mode
        .map(|mode| mode.requires_isolated_worktree())
        .unwrap_or(false)
    {
        Some(create_isolated_agent_worktree(context, description).await?)
    } else {
        None
    };
    let execution_working_dir = isolated_worktree
        .as_ref()
        .map(|worktree| worktree.path.as_path())
        .unwrap_or(context.working_dir.as_path());
    let file_context = load_file_context(files, execution_working_dir).await;
    let mut system_prompt = build_system_prompt(template, role, description, prompt, &file_context);
    if let Some(definition) = definition {
        if !definition.system_prompt.trim().is_empty() {
            system_prompt = format!("{}\n\n{}", definition.system_prompt.trim(), system_prompt);
        }
        let mut contract_lines = definition.contract_lines();
        if !definition.when_to_use.trim().is_empty() {
            contract_lines.push(format!("When to use: {}", definition.when_to_use));
        }
        if !contract_lines.is_empty() {
            system_prompt = format!(
                "Sub-agent definition contract:\n{}\n\n{}",
                contract_lines.join("\n"),
                system_prompt
            );
        }
    } else if let Some(context_mode) = effective_context_mode {
        system_prompt = format!(
            "Sub-agent definition contract:\nContext mode: {}\n\n{}",
            context_mode, system_prompt
        );
    }
    let should_build_fork_context = force_fork_context
        || effective_context_mode
            .map(|mode| mode.copies_full_history())
            .unwrap_or(false);
    let forked_context = if should_build_fork_context {
        if crate::agent::forked_context::text_contains_fork_boilerplate(description)
            || crate::agent::forked_context::text_contains_fork_boilerplate(prompt)
        {
            return Err(anyhow::anyhow!(
                "recursive fork blocked: task already contains fork boilerplate"
            ));
        }
        let mut request = crate::agent::forked_context::ForkedContextBuildRequest::new(
            prompt,
            context.parent_assistant_tool_calls.clone(),
        )
        .with_parent_assistant_content(context.parent_assistant_content.clone());
        if let Some(worktree) = isolated_worktree.as_ref() {
            request =
                request.with_worktree_notice(crate::agent::forked_context::build_worktree_notice(
                    &context.working_dir,
                    &worktree.path,
                ));
        }
        let built = crate::agent::forked_context::build_forked_context(request)
            .map_err(|err| anyhow::anyhow!(err))?;
        system_prompt = format!(
            "Forked context contract:\nplaceholder_tool_results={}\nparent_tool_call_ids={}\n\n{}",
            built.placeholder_result,
            if built.tool_call_ids.is_empty() {
                "none".to_string()
            } else {
                built.tool_call_ids.join(",")
            },
            system_prompt
        );
        Some(built)
    } else {
        None
    };
    system_prompt = append_subagent_tool_authorization_notice(
        system_prompt,
        &exposed_tools,
        isolated_worktree.is_some(),
    );

    let agent_config = AgentConfig::new(format!("sub-agent: {}", description))
        .with_description(description)
        .with_system_prompt(system_prompt)
        .with_max_turns(max_turns)
        .with_subagent_session_policy(subagent_session_policy.clone())
        .with_working_dir(execution_working_dir.to_path_buf())
        .with_mcp_servers(
            definition
                .map(|definition| definition.mcp_servers.clone())
                .unwrap_or_default(),
        )
        .with_context_messages(
            forked_context
                .as_ref()
                .map(|context| context.messages.clone())
                .unwrap_or_default(),
        );
    let agent_config = if let Some(store) = context.session_store.as_ref() {
        ensure_child_session(
            store,
            &child_session_id,
            description,
            &context.session_id,
            if context.model.trim().is_empty() {
                "unknown"
            } else {
                context.model.as_str()
            },
            Some(&context.working_dir.to_string_lossy()),
        );
        let agent_config = agent_config.with_child_session(store.clone(), child_session_id.clone());
        agent_config.with_completion_sink(AgentCompletionSink {
            store: store.clone(),
            parent_session_id: context.session_id.clone(),
            task_id: task_id.clone(),
            profile: definition.map(|definition| definition.name.clone()),
            role: role.display_name().to_string(),
            description: description.to_string(),
        })
    } else {
        agent_config
    };
    let agent_config = if let Some(limit) = max_cost_usd {
        agent_config.with_max_cost_usd(limit)
    } else {
        agent_config
    }
    .with_role(role);

    info!("Spawning sub-agent for task: {}", description);

    let agent_id = agent_manager.spawn(agent_config, None).await?;
    info!("Sub-agent spawned: {}", agent_id);
    let task_payload = json!({
        "task_id": task_id,
        "parent_session_id": context.session_id,
        "child_session_id": child_session_id,
        "timeout_secs": timeout_secs,
        "max_turns": max_turns,
        "requested_allowed_tools": allowed_tools,
        "allowed_tools": exposed_tools.clone(),
        "subagent_session_policy": subagent_session_policy.payload(),
        "context_mode": effective_context_mode.map(|mode| mode.to_string()),
        "isolated_worktree": isolated_worktree.as_ref().map(|worktree| json!({
            "path": worktree.path.to_string_lossy().to_string(),
            "branch": worktree.branch.clone(),
        })),
        "fork_context": forked_context.as_ref().map(|fork| json!({
            "message_count": fork.messages.len(),
            "placeholder_complete": fork.is_placeholder_complete(),
            "tool_call_ids": fork.tool_call_ids.clone(),
        })),
    });
    persist_agent_task_state(
        context,
        &task_id,
        &agent_id,
        description,
        role,
        definition,
        "running",
        None,
        task_payload.clone(),
    );
    if let Some(trace) = context.trace_collector.as_ref() {
        trace.record(crate::engine::trace::TraceEvent::SubagentStarted {
            agent_id: agent_id.to_string(),
            profile: definition.map(|definition| definition.name.clone()),
            role: role.display_name().to_string(),
            description: description.to_string(),
            timeout_secs,
            allowed_tools: exposed_tools.len(),
        });
    }

    let mut envelope = AgentTaskEnvelope::new(
        AgentId("parent".to_string()),
        description.to_string(),
        prompt.to_string(),
    )
    .assign_to(agent_id.clone())
    .with_priority(AgentTaskPriority::Normal);
    for file in files {
        envelope.add_context_ref(file.clone());
    }
    envelope.add_expected_artifact("task_result");
    envelope.add_constraint(format!("task_id={}", task_id));
    envelope.add_context_ref(format!("child_session_id={}", child_session_id));
    envelope.add_constraint(format!("timeout_secs={}", timeout_secs));
    envelope.add_constraint(format!("max_turns={}", max_turns));
    if !exposed_tools.is_empty() {
        envelope.add_constraint(format!("allowed_tools={}", exposed_tools.join(",")));
    }
    if let Some(definition) = definition {
        envelope.add_constraint(format!("profile={}", definition.name));
        for constraint in definition.envelope_constraints() {
            envelope.add_constraint(constraint);
        }
        envelope.add_expected_artifact(definition.output_contract.to_string());
    }
    let envelope_json = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|_| "{\"error\":\"failed to serialize envelope\"}".to_string());
    info!("Sub-agent task envelope: {}", envelope.compact_summary());
    let _ = crate::agent::a2a_transcript::append_envelope(&envelope);

    let task_msg = AgentMessage::new(
        AgentId("parent".to_string()),
        agent_id.clone(),
        format!(
            "<agent-task-envelope>\n{}\n</agent-task-envelope>\n\n{}",
            envelope_json, prompt
        ),
        AgentMessageType::Task,
    );

    agent_manager
        .send_message(&agent_id, task_msg)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send task: {}", e))?;

    if !wait_for_completion {
        return Ok(ManagerAgentResult {
            agent_id,
            status: AgentStatus::Running,
            content: format!(
                "Sub-agent launched in background for task_id {}. Completion will be persisted by the runtime sink.",
                task_id
            ),
            completed_at: std::time::Instant::now(),
            tools_used: Vec::new(),
            confidence: 0.0,
            has_conflict: false,
        });
    }

    info!(
        "Waiting for sub-agent {} to complete (timeout: {}s)...",
        agent_id, timeout_secs
    );

    let result = match agent_manager.wait_for_result(&agent_id, timeout_secs).await {
        Ok(result) => Ok(result),
        Err(error) => {
            match recover_agent_result_from_durable_sink(context, &task_id, &agent_id).await {
                Some(result) => Ok(result),
                None => Err(error),
            }
        }
    };
    if let Some(trace) = context.trace_collector.as_ref() {
        match &result {
            Ok(result) => trace.record(crate::engine::trace::TraceEvent::SubagentCompleted {
                agent_id: result.agent_id.to_string(),
                status: format!("{:?}", result.status).to_ascii_lowercase(),
                duration_ms: started_at.elapsed().as_millis() as u64,
                output_chars: result.content.chars().count(),
                tools_used: result.tools_used.len(),
            }),
            Err(_) => trace.record(crate::engine::trace::TraceEvent::SubagentCompleted {
                agent_id: agent_id.to_string(),
                status: "failed".to_string(),
                duration_ms: started_at.elapsed().as_millis() as u64,
                output_chars: 0,
                tools_used: 0,
            }),
        }
    }
    if let Err(error) = &result {
        let mut failure_payload = task_payload;
        failure_payload["error"] = json!(error.to_string());
        persist_agent_task_state(
            context,
            &task_id,
            &agent_id,
            description,
            role,
            definition,
            agent_wait_failure_status(error),
            None,
            failure_payload,
        );
    }
    result
}

async fn recover_agent_result_from_durable_sink(
    context: &ToolContext,
    task_id: &str,
    agent_id: &AgentId,
) -> Option<ManagerAgentResult> {
    let store = context.session_store.as_ref()?;
    for _ in 0..20 {
        let state = store
            .agent_task_state(&context.session_id, task_id)
            .ok()
            .flatten()
            .or_else(|| {
                store
                    .agent_task_state(&context.session_id, &agent_id.to_string())
                    .ok()
                    .flatten()
            })?;
        let artifact_id = state.result_artifact_id;
        if let Some(artifact_id) = artifact_id {
            if let Ok(Some(artifact)) = store.agent_artifact(&context.session_id, artifact_id) {
                let status = agent_status_from_durable(&state.status);
                return Some(ManagerAgentResult {
                    agent_id: AgentId(state.agent_id),
                    status,
                    content: artifact.output,
                    completed_at: std::time::Instant::now(),
                    tools_used: durable_tools_used(&artifact.payload)
                        .or_else(|| durable_tools_used(&state.payload))
                        .unwrap_or_default(),
                    confidence: artifact
                        .payload
                        .get("confidence")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(1.0) as f32,
                    has_conflict: artifact
                        .payload
                        .get("has_conflict")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                });
            }
        }
        if !matches!(state.status.as_str(), "running") {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    None
}

fn agent_status_from_durable(status: &str) -> AgentStatus {
    match status {
        "completed" => AgentStatus::Completed,
        "failed" => AgentStatus::Failed,
        "cancelled" => AgentStatus::Cancelled,
        "timed_out" => AgentStatus::Failed,
        "paused_restart" => AgentStatus::Failed,
        _ => AgentStatus::Running,
    }
}

fn durable_tools_used(value: &serde_json::Value) -> Option<Vec<String>> {
    let tools = value.get("tools_used")?;
    let items = tools.as_array()?;
    Some(
        items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

fn append_subagent_tool_authorization_notice(
    system_prompt: String,
    allowed_tools: &[String],
    isolated_worktree: bool,
) -> String {
    if allowed_tools.is_empty() {
        return system_prompt;
    }

    let scope = if isolated_worktree {
        "inside this isolated worktree"
    } else {
        "inside this sub-agent task"
    };
    format!(
        "{}\n\nSub-agent tool authorization:\n\
- The parent runtime has explicitly exposed only these tools for this task: {}.\n\
- When the directive requires one of these tools, call the tool directly {}; do not ask the user for confirmation solely because the tool is mutating.\n\
- Runtime permission and scope gates still enforce safety. If an exposed tool is rejected, report that concrete blocker with the rejection evidence.",
        system_prompt,
        allowed_tools.join(","),
        scope
    )
}

/// 汇总多个子 Agent 结果
pub(super) fn synthesize_results(
    description: &str,
    results: Vec<ManagerAgentResult>,
    files: &[String],
    role: AgentRole,
    template: Option<AgentTemplate>,
    allowed_tools: &[String],
) -> ToolResult {
    let files_info = if files.is_empty() {
        String::new()
    } else {
        format!("\nRelevant files: {}", files.join(", "))
    };

    let mut output = format!(
        "Parallel sub-agents completed for task: {}{}\n\n",
        description, files_info
    );

    let success_count = results
        .iter()
        .filter(|r| r.status == crate::agent::types::AgentStatus::Completed)
        .count();
    let fail_count = results.len() - success_count;

    output.push_str(&format!(
        "Summary: {} succeeded, {} failed (total: {})\n\n",
        success_count,
        fail_count,
        results.len()
    ));

    for (i, result) in results.iter().enumerate() {
        let status_label = if result.status == crate::agent::types::AgentStatus::Completed {
            "✓ SUCCESS"
        } else {
            "✗ FAILED"
        };
        output.push_str(&format!(
            "--- Agent {} ({}) ---\n{status_label}\n{}\n\n",
            i + 1,
            result.agent_id,
            result.content
        ));
    }

    let mut result_items = Vec::with_capacity(results.len());
    for result in &results {
        let mut item = json!({
            "agent_id": result.agent_id.to_string(),
            "status": format!("{:?}", result.status).to_lowercase(),
            "content": result.content.clone(),
        });
        attach_subagent_proof_metadata(&mut item, result, role, template, allowed_tools, false);
        result_items.push(item);
    }

    let mut data = json!({
        "description": description,
        "total": results.len(),
        "succeeded": success_count,
        "failed": fail_count,
        "results": result_items,
        "files": files,
    });
    if let Some(first_result) = results.first() {
        attach_subagent_proof_metadata(
            &mut data,
            first_result,
            role,
            template,
            allowed_tools,
            false,
        );
        data["scope"] = json!("subagent_result_set");
    }

    ToolResult::success_with_data(output, data)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn persist_agent_task_state(
    context: &ToolContext,
    task_id: &str,
    agent_id: &AgentId,
    description: &str,
    role: AgentRole,
    definition: Option<&AgentDefinition>,
    status: &str,
    result_artifact_id: Option<i64>,
    payload: serde_json::Value,
) {
    let Some(store) = context.session_store.as_ref() else {
        return;
    };
    let requires_worktree_cleanup = definition
        .map(|definition| definition.context_mode.requires_isolated_worktree())
        .unwrap_or(false)
        || payload.get("isolated_worktree").is_some();
    let cleanup_hooks = if requires_worktree_cleanup {
        vec!["worktree_cleanup".to_string()]
    } else {
        Vec::new()
    };
    let mut payload = payload;
    if let Some(definition) = definition {
        payload["agent_definition"] = json!({
            "name": definition.name.clone(),
            "agent_type": definition.agent_type.clone(),
            "context_mode": definition.context_mode.to_string(),
            "permission_mode": definition.permission_mode.to_string(),
            "risk_policy": definition.risk_policy.to_string(),
            "output_contract": definition.output_contract.to_string(),
            "memory_policy": definition.memory_policy.to_string(),
            "model": definition.model_policy.model.clone(),
            "mcp_servers": definition.mcp_servers.clone(),
        });
    }
    let state = crate::session_store::AgentTaskStateUpsert {
        session_id: context.session_id.clone(),
        task_id: task_id.to_string(),
        agent_id: agent_id.to_string(),
        profile: definition.map(|definition| definition.name.clone()),
        role: role.display_name().to_string(),
        status: status.to_string(),
        description: description.to_string(),
        transcript_path: Some(
            crate::agent::a2a_transcript::transcript_path()
                .to_string_lossy()
                .to_string(),
        ),
        tool_ids_in_progress: Vec::new(),
        permission_requests: Vec::new(),
        result_artifact_id,
        cleanup_hooks,
        payload,
    };
    if let Err(err) = store.upsert_agent_task_state(&state) {
        warn!(
            "Failed to persist sub-agent task state for {}: {}",
            agent_id, err
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_tool_authorization_notice_tells_worker_to_call_allowed_tools() {
        let prompt = append_subagent_tool_authorization_notice(
            "base".to_string(),
            &["file_write".to_string(), "bash".to_string()],
            true,
        );

        assert!(prompt.contains("file_write,bash"));
        assert!(prompt.contains("call the tool directly inside this isolated worktree"));
        assert!(prompt
            .contains("do not ask the user for confirmation solely because the tool is mutating"));
        assert!(prompt.contains("Runtime permission and scope gates still enforce safety"));
    }

    #[test]
    fn subagent_proof_metadata_exposes_runtime_tools_used() {
        let result = ManagerAgentResult {
            agent_id: AgentId("agent_1".to_string()),
            status: AgentStatus::Completed,
            content: "done".to_string(),
            completed_at: std::time::Instant::now(),
            tools_used: vec!["file_write".to_string(), "file_read".to_string()],
            confidence: 1.0,
            has_conflict: false,
        };
        let mut data = serde_json::json!({});

        attach_subagent_proof_metadata(
            &mut data,
            &result,
            AgentRole::Specialist,
            None,
            &["file_write".to_string()],
            false,
        );

        assert_eq!(data["tools_used"][0], "file_write");
        assert_eq!(data["tools_used"][1], "file_read");
    }

    #[tokio::test]
    async fn durable_sink_result_recovers_after_foreground_wait_timeout() {
        let store = std::sync::Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store
            .create_session("parent", "Parent", "model", Some("/repo"))
            .unwrap();
        let artifact_id = store
            .add_agent_artifact(
                "parent",
                "agent_1",
                Some("implementer"),
                "specialist",
                "completed",
                "edit code",
                "done",
                &serde_json::json!({
                    "tools_used": ["file_write", "file_read"],
                    "confidence": 0.7,
                    "has_conflict": false,
                    "completion_sink": "agent_manager",
                }),
            )
            .unwrap();
        store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "parent".to_string(),
                task_id: "task_1".to_string(),
                agent_id: "agent_1".to_string(),
                profile: Some("implementer".to_string()),
                role: "specialist".to_string(),
                status: "completed".to_string(),
                description: "edit code".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({}),
            })
            .unwrap();

        let result = recover_agent_result_from_durable_sink(
            &ToolContext::new(".", "parent").with_session_store(store),
            "task_1",
            &AgentId("agent_1".to_string()),
        )
        .await
        .expect("durable artifact should recover agent result");

        assert_eq!(result.status, AgentStatus::Completed);
        assert_eq!(result.content, "done");
        assert_eq!(
            result.tools_used,
            vec!["file_write".to_string(), "file_read".to_string()]
        );
        assert_eq!(result.confidence, 0.7);
    }
}

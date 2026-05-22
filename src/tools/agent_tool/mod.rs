//! Agent 工具 - 创建子 Agent
//!
//! 用于创建并委派任务给子 Agent

use crate::agent::agent::AgentConfig;
use crate::agent::envelope::{AgentTaskEnvelope, AgentTaskPriority};
use crate::agent::manager::AgentResult as ManagerAgentResult;
use crate::agent::profiles::{AgentContextMode, AgentDefinition, AgentProfile};
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 子代理模板
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentTemplate {
    /// 代码探索：分析结构、依赖关系、关键函数
    Explore,
    /// 验证/审查：检查正确性、潜在 bug、安全漏洞
    Verify,
    /// 任务规划：分解步骤、排序依赖、制定计划
    Plan,
    /// 通用任务：灵活处理各种任务
    GeneralPurpose,
    /// 代码审查：专注于代码质量和最佳实践
    CodeReview,
    /// 调试：系统性定位和修复问题
    Debug,
}

impl AgentTemplate {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "explore" => Some(Self::Explore),
            "verify" => Some(Self::Verify),
            "plan" => Some(Self::Plan),
            "general" | "generalpurpose" | "general_purpose" => Some(Self::GeneralPurpose),
            "review" | "codereview" | "code_review" => Some(Self::CodeReview),
            "debug" | "debugging" => Some(Self::Debug),
            _ => None,
        }
    }

    fn build_system_prompt(&self, description: &str, prompt: &str) -> String {
        match self {
            AgentTemplate::Explore => {
                format!(
                    "You are an EXPLORATION sub-agent. Your job is to deeply understand code or data.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Focus on:\n\
                     - High-level architecture and structure\n\
                     - Key functions, modules, and their relationships\n\
                     - Dependencies and data flow\n\
                     - Notable patterns or abnormalities\n\n\
                     Be thorough but organized. Use bullet points and sections.",
                    description, prompt
                )
            }
            AgentTemplate::Verify => {
                format!(
                    "You are a VERIFICATION sub-agent. Your job is to critically review code or plans.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Focus on:\n\
                     - Correctness and edge cases\n\
                     - Potential bugs or logical errors\n\
                     - Security or safety issues\n\
                     - Performance concerns\n\
                     - Whether the implementation matches the stated intent\n\n\
                     If you find issues, explain them clearly with line references where possible. If everything looks good, say so explicitly.",
                    description, prompt
                )
            }
            AgentTemplate::Plan => {
                format!(
                    "You are a PLANNING sub-agent. Your job is to break down complex tasks into actionable steps.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Output a clear plan with:\n\
                     1. Goal restatement\n\
                     2. Ordered steps (with dependencies noted)\n\
                     3. Estimated complexity for each step\n\
                     4. Potential risks and how to mitigate them\n\
                     5. Success criteria\n\n\
                     Be concrete. Avoid vague language.",
                    description, prompt
                )
            }
            AgentTemplate::GeneralPurpose => {
                format!(
                    "You are a GENERAL PURPOSE sub-agent. Your job is to handle a wide variety of tasks efficiently.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Guidelines:\n\
                     - Be thorough but concise\n\
                     - Ask clarifying questions if the task is ambiguous\n\
                     - Provide actionable outputs\n\
                     - Document your reasoning\n\
                     - Suggest next steps if appropriate\n\n\
                     Adapt your approach based on the task type (coding, analysis, writing, etc.).",
                    description, prompt
                )
            }
            AgentTemplate::CodeReview => {
                format!(
                    "You are a CODE REVIEW sub-agent. Your job is to review code for quality, correctness, and best practices.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Review checklist:\n\
                     - Correctness: Does the code do what it claims?\n\
                     - Edge cases: Are boundary conditions handled?\n\
                     - Error handling: Are errors properly caught and handled?\n\
                     - Performance: Are there obvious inefficiencies?\n\
                     - Security: Are there potential vulnerabilities?\n\
                     - Readability: Is the code clear and well-documented?\n\
                     - Best practices: Does it follow language/framework conventions?\n\n\
                     Provide specific line references for issues found. Rate overall quality (1-10).",
                    description, prompt
                )
            }
            AgentTemplate::Debug => {
                format!(
                    "You are a DEBUGGING sub-agent. Your job is to systematically identify and fix bugs.\n\
                     Task: {}\n\n\
                     Instructions: {}\n\n\
                     Debugging methodology:\n\
                     1. Reproduce: Confirm the bug exists and understand its behavior\n\
                     2. Isolate: Narrow down the location of the bug\n\
                     3. Hypothesize: Form theories about the root cause\n\
                     4. Test: Verify hypotheses with targeted tests\n\
                     5. Fix: Implement the minimal fix that addresses the root cause\n\
                     6. Verify: Confirm the fix works and doesn't introduce regressions\n\n\
                     Document your debugging process and reasoning.",
                    description, prompt
                )
            }
        }
    }
}

/// 子任务定义
#[derive(Debug, Clone)]
struct SubTask {
    description: String,
    prompt: String,
    files: Vec<String>,
}

/// Agent 工具 - 创建子 Agent 执行复杂任务
pub struct AgentTool;

/// 加载文件上下文
async fn load_file_context(files: &[String], working_dir: &Path) -> String {
    let mut context = String::new();
    for file in files {
        let path = working_dir.join(file);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                context.push_str(&format!("\n## File: {}\n```\n{}\n```\n", file, content));
            }
            Err(e) => {
                warn!("Failed to load context file {}: {}", path.display(), e);
                context.push_str(&format!(
                    "\n## File: {}\n(error loading file: {})\n",
                    file, e
                ));
            }
        }
    }
    context
}

/// 构建完整的系统提示词
fn build_system_prompt(
    template: Option<AgentTemplate>,
    role: AgentRole,
    description: &str,
    prompt: &str,
    file_context: &str,
) -> String {
    let role_prefix = role.system_prompt_prefix();
    let base_prompt = match template {
        Some(t) => t.build_system_prompt(description, prompt),
        None => format!(
            "You are a sub-agent tasked with: {}\n\n\
             Instructions: {}\n\n\
             Complete this task and report back with your findings. Be thorough and detailed in your response.",
            description, prompt
        ),
    };

    if file_context.is_empty() {
        format!("{}\n\n{}", role_prefix, base_prompt)
    } else {
        format!(
            "{}\n\n{}\n\nRelevant files are provided below:\n{}",
            role_prefix, base_prompt, file_context
        )
    }
}

#[derive(Debug, Clone)]
struct IsolatedAgentWorktree {
    path: PathBuf,
    branch: String,
}

async fn create_isolated_agent_worktree(
    context: &ToolContext,
    description: &str,
) -> anyhow::Result<IsolatedAgentWorktree> {
    let manager = context
        .worktree_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("isolated_worktree_fork requires a WorktreeManager"))?;
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let suffix = &uuid[..8];
    let slug = isolated_worktree_slug(description);
    let name = format!("agent-{}-{}", slug, suffix);
    let branch = format!("codex/agent-{}", suffix);
    let path = manager.create(&name, Some(&branch)).await?;
    Ok(IsolatedAgentWorktree { path, branch })
}

fn isolated_worktree_slug(description: &str) -> String {
    let mut slug = String::new();
    for ch in description.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= 32 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "worker".to_string()
    } else {
        slug.to_string()
    }
}

fn tools_allow_file_mutation(tools: &[String]) -> bool {
    tools
        .iter()
        .any(|tool| matches!(tool.as_str(), "file_edit" | "file_write" | "apply_patch"))
}

fn effective_agent_context_mode(
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

fn agent_wait_failure_status(error: &anyhow::Error) -> &'static str {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timeout") || message.contains("timed out") {
        "timed_out"
    } else {
        "failed"
    }
}

/// 创建并等待单个子 Agent 完成
#[allow(clippy::too_many_arguments)]
async fn spawn_single_agent(
    agent_manager: &crate::agent::AgentManager,
    description: &str,
    prompt: &str,
    files: &[String],
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
    let effective_context_mode =
        effective_agent_context_mode(context_mode_override, definition, allowed_tools);
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

    let agent_config = AgentConfig::new(format!("sub-agent: {}", description))
        .with_description(description)
        .with_system_prompt(system_prompt)
        .with_max_turns(max_turns)
        .with_allowed_tools(allowed_tools.to_vec())
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
        "timeout_secs": timeout_secs,
        "max_turns": max_turns,
        "allowed_tools": allowed_tools,
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
            allowed_tools: allowed_tools.len(),
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
    envelope.add_constraint(format!("timeout_secs={}", timeout_secs));
    envelope.add_constraint(format!("max_turns={}", max_turns));
    if !allowed_tools.is_empty() {
        envelope.add_constraint(format!("allowed_tools={}", allowed_tools.join(",")));
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

    info!(
        "Waiting for sub-agent {} to complete (timeout: {}s)...",
        agent_id, timeout_secs
    );

    let result = agent_manager.wait_for_result(&agent_id, timeout_secs).await;
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

/// 汇总多个子 Agent 结果
fn synthesize_results(
    description: &str,
    results: Vec<ManagerAgentResult>,
    files: &[String],
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

    let data = json!({
        "description": description,
        "total": results.len(),
        "succeeded": success_count,
        "failed": fail_count,
        "results": results.iter().map(|r| json!({
            "agent_id": r.agent_id.to_string(),
            "status": format!("{:?}", r.status).to_lowercase(),
            "content": r.content,
        })).collect::<Vec<_>>(),
        "files": files,
    });

    ToolResult::success_with_data(output, data)
}

#[allow(clippy::too_many_arguments)]
fn persist_agent_task_state(
    context: &ToolContext,
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
        task_id: agent_id.to_string(),
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

fn persist_agent_artifact(
    context: &ToolContext,
    description: &str,
    role: AgentRole,
    definition: Option<&AgentDefinition>,
    result: &ManagerAgentResult,
) {
    let Some(store) = context.session_store.as_ref() else {
        return;
    };
    let status = format!("{:?}", result.status).to_ascii_lowercase();
    let payload = json!({
        "tools_used": result.tools_used,
        "confidence": result.confidence,
        "has_conflict": result.has_conflict,
    });
    let artifact_id = match store.add_agent_artifact(
        &context.session_id,
        &result.agent_id.to_string(),
        definition.map(|definition| definition.name.as_str()),
        role.display_name(),
        &status,
        description,
        &result.content,
        &payload,
    ) {
        Ok(id) => Some(id),
        Err(err) => {
            warn!(
                "Failed to persist sub-agent artifact for {}: {}",
                result.agent_id, err
            );
            None
        }
    };
    persist_agent_task_state(
        context,
        &result.agent_id,
        description,
        role,
        definition,
        &status,
        artifact_id,
        payload,
    );
}

/// 执行上下文参数
struct ExecuteParams<'a> {
    agent_manager: &'a crate::agent::AgentManager,
    params: &'a serde_json::Value,
    context: &'a ToolContext,
    timeout_secs: u64,
    max_turns: usize,
    max_cost_usd: Option<f64>,
    allowed_tools: Vec<String>,
    role: AgentRole,
    template: Option<AgentTemplate>,
    definition: Option<AgentDefinition>,
    context_mode_override: Option<AgentContextMode>,
}

fn default_subagent_allowed_tools(role: AgentRole, template: Option<AgentTemplate>) -> Vec<String> {
    let names: &[&str] = match template {
        Some(AgentTemplate::Explore) => &["project_list", "glob", "grep", "file_read"],
        Some(AgentTemplate::Verify) | Some(AgentTemplate::CodeReview) => {
            &["project_list", "glob", "grep", "file_read", "bash"]
        }
        Some(AgentTemplate::Plan) => &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "plan",
            "todo_write",
        ],
        Some(AgentTemplate::Debug) => &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "file_edit",
            "file_write",
            "bash",
            "diff",
            "format",
        ],
        Some(AgentTemplate::GeneralPurpose) | None => match role {
            AgentRole::Plan | AgentRole::Advisor | AgentRole::Guide => &[
                "project_list",
                "glob",
                "grep",
                "file_read",
                "plan",
                "todo_write",
            ],
            AgentRole::Verification => &["project_list", "glob", "grep", "file_read", "bash"],
            AgentRole::Specialist | AgentRole::Fast | AgentRole::Teammate | AgentRole::Default => {
                &[
                    "project_list",
                    "glob",
                    "grep",
                    "file_read",
                    "file_edit",
                    "file_write",
                    "bash",
                    "diff",
                    "format",
                ]
            }
            AgentRole::DreamTask => &[
                "project_list",
                "glob",
                "grep",
                "file_read",
                "plan",
                "todo_write",
            ],
        },
    };
    names.iter().map(|name| (*name).to_string()).collect()
}

fn push_unique_tool(tools: &mut Vec<String>, tool: impl Into<String>) {
    let tool = tool.into();
    if !tools.iter().any(|item| item == &tool) {
        tools.push(tool);
    }
}

fn resolve_subagent_allowed_tools(
    requested_tools: Vec<String>,
    profile: Option<&AgentProfile>,
    definition: Option<&AgentDefinition>,
    role: AgentRole,
    template: Option<AgentTemplate>,
) -> Vec<String> {
    let mut tools = if !requested_tools.is_empty() {
        requested_tools
    } else {
        profile
            .filter(|profile| !profile.allowed_tools.is_empty())
            .map(|profile| profile.allowed_tools.clone())
            .unwrap_or_else(|| default_subagent_allowed_tools(role, template))
    };
    let Some(definition) = definition else {
        let mut deduped = Vec::with_capacity(tools.len());
        for tool in tools {
            push_unique_tool(&mut deduped, tool);
        }
        return deduped;
    };
    if !definition.mcp_servers.is_empty() {
        push_unique_tool(&mut tools, "mcp_tool");
        push_unique_tool(&mut tools, "list_mcp_resources");
        push_unique_tool(&mut tools, "read_mcp_resource");
    }
    if !definition.disallowed_tools.is_empty() {
        tools.retain(|tool| {
            !definition
                .disallowed_tools
                .iter()
                .any(|blocked| blocked == tool)
        });
    }
    let mut deduped = Vec::with_capacity(tools.len());
    for tool in tools {
        push_unique_tool(&mut deduped, tool);
    }
    deduped
}

/// 处理恢复已有代理
async fn handle_resume(
    agent_manager: &crate::agent::AgentManager,
    agent_id_str: &str,
) -> ToolResult {
    let agent_id = AgentId(agent_id_str.to_string());
    match agent_manager.get_result(&agent_id).await {
        Some(result) => {
            let status_str = format!("{:?}", result.status);
            ToolResult::success_with_data(
                format!(
                    "Resumed agent {}\nStatus: {}\n\nResult:\n{}",
                    agent_id, status_str, result.content
                ),
                json!({
                    "agent_id": agent_id.to_string(),
                    "status": status_str.to_lowercase(),
                    "result": result.content,
                    "resumed": true,
                }),
            )
        }
        None => ToolResult::error(format!(
            "Agent {} not found or has no result yet",
            agent_id_str
        )),
    }
}

fn format_durable_agent_read(
    state: &crate::session_store::AgentTaskStateRecord,
    artifact: Option<&crate::session_store::AgentArtifactRecord>,
) -> String {
    let mut lines = vec![
        format!("Sub-agent {}", state.agent_id),
        format!("Status: {}", state.status),
        format!("Task: {}", state.description),
        format!("Role: {}", state.role),
    ];
    if let Some(profile) = state.profile.as_deref() {
        lines.push(format!("Profile: {}", profile));
    }
    if let Some(path) = state.transcript_path.as_deref() {
        lines.push(format!("Transcript: {}", path));
    }
    if !state.cleanup_hooks.is_empty() {
        lines.push(format!("Cleanup: {}", state.cleanup_hooks.join(",")));
    }
    if !state.permission_requests.is_empty() {
        lines.push(format!(
            "Permission requests: {}",
            state.permission_requests.join(",")
        ));
    }
    if let Some(artifact) = artifact {
        lines.push(String::new());
        lines.push(format!(
            "Result artifact {} [{}]:",
            artifact.id, artifact.status
        ));
        lines.push(artifact.output.clone());
    } else if state.result_artifact_id.is_some() {
        lines.push("Result artifact: missing in current session store".to_string());
    } else {
        lines.push("Result artifact: none yet".to_string());
    }
    lines.join("\n")
}

fn read_durable_agent_state(context: &ToolContext, agent_id: &str) -> ToolResult {
    let Some(store) = context.session_store.as_ref() else {
        return ToolResult::error(
            "Session store not available. Durable agent read requires session state.",
        );
    };
    let state = match store.agent_task_state(&context.session_id, agent_id) {
        Ok(Some(state)) => state,
        Ok(None) => {
            return ToolResult::error(format!(
                "Agent task '{}' was not found in current session {}",
                agent_id, context.session_id
            ));
        }
        Err(error) => {
            return ToolResult::error(format!("Failed to read agent task state: {}", error))
        }
    };
    let artifact = match state.result_artifact_id {
        Some(id) => match store.agent_artifact(&context.session_id, id) {
            Ok(artifact) => artifact,
            Err(error) => {
                return ToolResult::error(format!(
                    "Failed to read agent artifact {}: {}",
                    id, error
                ));
            }
        },
        None => None,
    };
    ToolResult::success_with_data(
        format_durable_agent_read(&state, artifact.as_ref()),
        json!({
            "agent_id": state.agent_id,
            "task_id": state.task_id,
            "status": state.status,
            "description": state.description,
            "profile": state.profile,
            "role": state.role,
            "transcript_path": state.transcript_path,
            "permission_requests": state.permission_requests,
            "cleanup_hooks": state.cleanup_hooks,
            "result_artifact_id": state.result_artifact_id,
            "artifact": artifact,
            "payload": state.payload,
        }),
    )
}

async fn list_agent_progress(
    context: &ToolContext,
    agent_manager: Option<&std::sync::Arc<crate::agent::AgentManager>>,
    limit: i64,
) -> ToolResult {
    let durable_states = match context.session_store.as_ref() {
        Some(store) => match store.recent_agent_task_states(&context.session_id, limit) {
            Ok(states) => states,
            Err(error) => {
                return ToolResult::error(format!("Failed to list agent task states: {}", error));
            }
        },
        None => Vec::new(),
    };
    let active_agents = match agent_manager {
        Some(manager) => manager.list_agents().await,
        None => Vec::new(),
    };

    let mut lines = vec![format!(
        "Sub-agent progress: {} durable task(s), {} active in-memory agent(s)",
        durable_states.len(),
        active_agents.len()
    )];
    if !active_agents.is_empty() {
        lines.push(String::new());
        lines.push("Active agents:".to_string());
        for handle in &active_agents {
            let status = format!("{:?}", *handle.status.borrow()).to_lowercase();
            lines.push(format!(
                "- {} [{}] {} - {}",
                handle.id, status, handle.config.name, handle.config.description
            ));
        }
    }
    if !durable_states.is_empty() {
        lines.push(String::new());
        lines.push("Durable task states:".to_string());
        for state in &durable_states {
            let mut suffix = Vec::new();
            if state.result_artifact_id.is_some() {
                suffix.push("artifact");
            }
            if !state.cleanup_hooks.is_empty() {
                suffix.push("cleanup");
            }
            let suffix = if suffix.is_empty() {
                String::new()
            } else {
                format!(" ({})", suffix.join(","))
            };
            lines.push(format!(
                "- {} / {} [{}] {}{}",
                state.agent_id, state.task_id, state.status, state.description, suffix
            ));
        }
    }
    if active_agents.is_empty() && durable_states.is_empty() {
        lines.push("No sub-agent activity recorded for this session.".to_string());
    }

    ToolResult::success_with_data(
        lines.join("\n"),
        json!({
            "active_agents": active_agents
                .iter()
                .map(|handle| {
                    json!({
                        "agent_id": handle.id.to_string(),
                        "status": format!("{:?}", *handle.status.borrow()).to_lowercase(),
                        "name": handle.config.name.clone(),
                        "description": handle.config.description.clone(),
                        "role": format!("{:?}", handle.config.role).to_lowercase(),
                        "working_dir": handle.config.working_dir.as_ref().map(|path| path.to_string_lossy().to_string()),
                    })
                })
                .collect::<Vec<_>>(),
            "durable_tasks": durable_states,
        }),
    )
}

fn cancelled_agent_task_state_upsert(
    state: &crate::session_store::AgentTaskStateRecord,
) -> crate::session_store::AgentTaskStateUpsert {
    let mut payload = state.payload.clone();
    payload["cancelled_at"] = json!(chrono::Utc::now().to_rfc3339());
    crate::session_store::AgentTaskStateUpsert {
        session_id: state.session_id.clone(),
        task_id: state.task_id.clone(),
        agent_id: state.agent_id.clone(),
        profile: state.profile.clone(),
        role: state.role.clone(),
        status: "cancelled".to_string(),
        description: state.description.clone(),
        transcript_path: state.transcript_path.clone(),
        tool_ids_in_progress: Vec::new(),
        permission_requests: state.permission_requests.clone(),
        result_artifact_id: state.result_artifact_id,
        cleanup_hooks: state.cleanup_hooks.clone(),
        payload,
    }
}

fn persist_cancelled_agent_task_state(context: &ToolContext, agent_id: &str) -> Option<String> {
    let store = context.session_store.as_ref()?;
    match store.agent_task_state(&context.session_id, agent_id) {
        Ok(Some(state)) => {
            let upsert = cancelled_agent_task_state_upsert(&state);
            if let Err(err) = store.upsert_agent_task_state(&upsert) {
                Some(format!("durable state update failed: {}", err))
            } else {
                Some("durable state updated to cancelled".to_string())
            }
        }
        Ok(None) => Some("durable state not found for this session".to_string()),
        Err(err) => Some(format!("durable state lookup failed: {}", err)),
    }
}

async fn handle_cancel(
    agent_manager: &crate::agent::AgentManager,
    context: &ToolContext,
    agent_id_str: &str,
) -> ToolResult {
    let agent_id = AgentId(agent_id_str.to_string());
    match agent_manager.kill(&agent_id).await {
        Ok(()) => {
            let durable_state = persist_cancelled_agent_task_state(context, agent_id_str)
                .unwrap_or_else(|| "durable state unavailable".to_string());
            ToolResult::success_with_data(
                format!("Cancelled sub-agent {}\n{}", agent_id, durable_state),
                json!({
                    "agent_id": agent_id.to_string(),
                    "status": "cancelled",
                    "durable_state": durable_state,
                }),
            )
        }
        Err(error) => ToolResult::error(format!(
            "Failed to cancel sub-agent {}: {}",
            agent_id, error
        )),
    }
}

/// 处理分叉分支探索
async fn handle_fork_branches(ctx: ExecuteParams<'_>) -> ToolResult {
    let branches_array = match ctx.params["fork_branches"].as_array() {
        Some(arr) => arr,
        None => return ToolResult::error("Invalid params: 'fork_branches' must be an array"),
    };
    if branches_array.is_empty() {
        return ToolResult::error("fork_branches array cannot be empty");
    }

    let description = ctx.params["description"]
        .as_str()
        .unwrap_or("fork exploration");
    let files: Vec<String> = ctx.params["files"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // If parent_agent_id specified, inherit its memory
    let parent_memory = if let Some(parent_id) = ctx.params["agent_id"].as_str() {
        let memory_manager = crate::agent::memory::global_memory_manager();
        Some(memory_manager.get_or_create(parent_id).await)
    } else {
        None
    };

    let mut results = Vec::new();
    for branch in branches_array {
        let branch_name = branch["branch_name"].as_str().unwrap_or("unnamed");
        let branch_prompt = branch["prompt"].as_str().unwrap_or("");
        let branch_desc = branch["description"].as_str().unwrap_or(branch_name);

        if branch_prompt.is_empty() {
            return ToolResult::error(format!(
                "Branch '{}' must have a non-empty prompt",
                branch_name
            ));
        }

        match spawn_single_agent(
            ctx.agent_manager,
            &format!("{}: {}", description, branch_desc),
            branch_prompt,
            &files,
            ctx.timeout_secs,
            ctx.max_turns,
            ctx.max_cost_usd,
            &ctx.allowed_tools,
            ctx.role,
            ctx.template,
            ctx.definition.as_ref(),
            ctx.context_mode_override,
            true,
            ctx.context,
        )
        .await
        {
            Ok(result) => {
                persist_agent_artifact(
                    ctx.context,
                    &format!("{}: {}", description, branch_desc),
                    ctx.role,
                    ctx.definition.as_ref(),
                    &result,
                );
                // If parent memory exists, copy to branch agent
                if let Some(ref parent_mem) = parent_memory {
                    let branch_memory = crate::agent::memory::global_memory_manager()
                        .get_or_create(&result.agent_id.to_string())
                        .await;
                    branch_memory.merge(parent_mem).await;
                }

                results.push(json!({
                    "branch_name": branch_name,
                    "agent_id": result.agent_id.to_string(),
                    "status": format!("{:?}", result.status).to_lowercase(),
                    "content": result.content,
                }));
            }
            Err(e) => {
                results.push(json!({
                    "branch_name": branch_name,
                    "error": e.to_string(),
                }));
            }
        }
    }

    let success_count = results
        .iter()
        .filter(|r| r.get("content").is_some())
        .count();
    let mut output = format!(
        "Fork exploration completed: {} branches\n\n",
        branches_array.len()
    );

    for r in &results {
        let branch_name = r["branch_name"].as_str().unwrap_or("unknown");
        if let Some(content) = r.get("content") {
            output.push_str(&format!(
                "=== Branch: {} ===\n{}\n\n",
                branch_name,
                content.as_str().unwrap_or("")
            ));
        } else if let Some(error) = r.get("error") {
            output.push_str(&format!(
                "=== Branch: {} (FAILED) ===\n{}\n\n",
                branch_name,
                error.as_str().unwrap_or("")
            ));
        }
    }

    ToolResult::success_with_data(
        output,
        json!({
            "description": description,
            "total_branches": branches_array.len(),
            "succeeded": success_count,
            "failed": branches_array.len() - success_count,
            "branches": results,
        }),
    )
}

/// 处理并行子任务
async fn handle_subtasks(ctx: ExecuteParams<'_>) -> ToolResult {
    let subtasks_array = match ctx.params["subtasks"].as_array() {
        Some(arr) => arr,
        None => return ToolResult::error("Invalid params: 'subtasks' must be an array"),
    };
    if subtasks_array.is_empty() {
        return ToolResult::error("subtasks array cannot be empty");
    }

    let mut subtasks = Vec::new();
    for t in subtasks_array {
        let desc = t["description"].as_str().unwrap_or("");
        let prompt = t["prompt"].as_str().unwrap_or("");
        if desc.is_empty() || prompt.is_empty() {
            return ToolResult::error("Each subtask must have a non-empty description and prompt");
        }
        let files: Vec<String> = t["files"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        subtasks.push(SubTask {
            description: desc.to_string(),
            prompt: prompt.to_string(),
            files,
        });
    }

    let description = ctx.params["description"]
        .as_str()
        .unwrap_or("parallel tasks");
    let parallel = ctx.params["parallel"].as_bool().unwrap_or(false);

    let results = if parallel || subtasks.len() > 1 {
        let futures: Vec<_> = subtasks
            .iter()
            .map(|st| {
                spawn_single_agent(
                    ctx.agent_manager,
                    &st.description,
                    &st.prompt,
                    &st.files,
                    ctx.timeout_secs,
                    ctx.max_turns,
                    ctx.max_cost_usd,
                    &ctx.allowed_tools,
                    ctx.role,
                    ctx.template,
                    ctx.definition.as_ref(),
                    ctx.context_mode_override,
                    false,
                    ctx.context,
                )
            })
            .collect();

        let completed = futures::future::join_all(futures).await;
        completed
            .into_iter()
            .filter_map(|r| r.ok())
            .inspect(|result| {
                persist_agent_artifact(
                    ctx.context,
                    description,
                    ctx.role,
                    ctx.definition.as_ref(),
                    result,
                );
            })
            .collect()
    } else {
        let st = &subtasks[0];
        match spawn_single_agent(
            ctx.agent_manager,
            &st.description,
            &st.prompt,
            &st.files,
            ctx.timeout_secs,
            ctx.max_turns,
            ctx.max_cost_usd,
            &ctx.allowed_tools,
            ctx.role,
            ctx.template,
            ctx.definition.as_ref(),
            ctx.context_mode_override,
            false,
            ctx.context,
        )
        .await
        {
            Ok(r) => {
                persist_agent_artifact(
                    ctx.context,
                    &st.description,
                    ctx.role,
                    ctx.definition.as_ref(),
                    &r,
                );
                vec![r]
            }
            Err(e) => {
                return ToolResult::error(format!("Sub-agent failed: {}", e));
            }
        }
    };

    let files: Vec<String> = ctx.params["files"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    synthesize_results(description, results, &files)
}

/// 处理单个代理
async fn handle_single_agent(ctx: ExecuteParams<'_>) -> ToolResult {
    let description = ctx.params["description"].as_str().unwrap_or("");
    let prompt = ctx.params["prompt"].as_str().unwrap_or("");
    if description.is_empty() {
        return ToolResult::error("Description cannot be empty");
    }
    if prompt.is_empty() {
        return ToolResult::error("Prompt cannot be empty");
    }

    let files: Vec<String> = ctx.params["files"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    match spawn_single_agent(
        ctx.agent_manager,
        description,
        prompt,
        &files,
        ctx.timeout_secs,
        ctx.max_turns,
        ctx.max_cost_usd,
        &ctx.allowed_tools,
        ctx.role,
        ctx.template,
        ctx.definition.as_ref(),
        ctx.context_mode_override,
        false,
        ctx.context,
    )
    .await
    {
        Ok(result) => {
            persist_agent_artifact(
                ctx.context,
                description,
                ctx.role,
                ctx.definition.as_ref(),
                &result,
            );
            let status_str = format!("{:?}", result.status);
            let files_info = if files.is_empty() {
                String::new()
            } else {
                format!("\nRelevant files: {}", files.join(", "))
            };
            let effective_context_mode = effective_agent_context_mode(
                ctx.context_mode_override,
                ctx.definition.as_ref(),
                &ctx.allowed_tools,
            );

            // Handle memory operations
            let memory_manager = crate::agent::memory::global_memory_manager();
            let agent_memory = memory_manager
                .get_or_create(&result.agent_id.to_string())
                .await;

            // If memory_key specified, save result to memory
            if let Some(memory_key) = ctx.params["memory_key"].as_str() {
                agent_memory.save(memory_key, &result.content).await;
                info!("Saved agent result to memory key: {}", memory_key);
            }

            // If memory_snapshot specified, create snapshot
            if ctx.params["memory_snapshot"].as_bool().unwrap_or(false) {
                let snapshot = agent_memory.snapshot().await;
                info!(
                    "Created memory snapshot at timestamp: {}",
                    snapshot.timestamp
                );
            }

            ToolResult::success_with_data(
                format!(
                    "Sub-agent {} completed with status: {}\n\nTask: {}{}\n\nResult:\n{}",
                    result.agent_id, status_str, description, files_info, result.content
                ),
                json!({
                    "agent_id": result.agent_id.to_string(),
                    "description": description,
                    "status": status_str.to_lowercase(),
                    "result": result.content,
                    "parent_session": ctx.context.session_id,
                    "files": files,
                    "role": ctx.role.display_name(),
                    "template": ctx.params["template"].as_str().unwrap_or(""),
                    "profile": ctx.definition.as_ref().map(|definition| definition.name.clone()),
                    "agent_definition": ctx.definition.as_ref(),
                    "context_mode": effective_context_mode.map(|mode| mode.to_string()),
                    "permission_mode": ctx.definition.as_ref().map(|definition| definition.permission_mode.to_string()),
                    "risk_policy": ctx.definition.as_ref().map(|definition| definition.risk_policy.to_string()),
                    "output_contract": ctx.definition.as_ref().map(|definition| definition.output_contract.to_string()),
                    "allowed_tools": ctx.allowed_tools,
                    "completed_at": result.completed_at.elapsed().as_secs()
                }),
            )
        }
        Err(e) => {
            warn!("Sub-agent failed or timed out: {}", e);
            ToolResult::error(format!("Sub-agent did not complete successfully: {}", e))
        }
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "agent"
    }

    fn description(&self) -> &str {
        "Delegate a task to a sub-agent with memory and fork support. \
         Use this for independent, parallel, non-blocking tasks, \
         or when a bounded task requires specialized context. \
         Do not delegate work that blocks the current next step or needs tight coordination. \
         The sub-agent will work independently and report back when done. \
         Built-in profiles: default, explorer, verifier, planner, implementer. \
         Built-in templates: explore, verify, plan, review, debug, general. \
         file context injection, agent memory (save/load/snapshot), and fork branches."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A clear description of what the sub-agent should do"
                },
                "prompt": {
                    "type": "string",
                    "description": "Detailed instructions for the sub-agent. \
                                 Be specific about what needs to be done, expected output, and any files it may change."
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of file paths that are relevant to this task"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum time to wait for sub-agent completion in seconds (default: 300)",
                    "default": 300
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum tool-calling turns the sub-agent may run (default: 10)",
                    "default": 10,
                    "minimum": 1
                },
                "max_cost_usd": {
                    "type": "number",
                    "description": "Optional max cost budget in USD for this sub-agent task",
                    "minimum": 0
                },
                "allowed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool whitelist for isolation. Set this for narrow tasks; if set, sub-agent can only call these tools."
                },
                "profile": {
                    "type": "string",
                    "description": "Named agent profile: default, explorer, verifier, planner, implementer, or a project profile from .priority-agent/agents/*.toml."
                },
                "context_mode": {
                    "type": "string",
                    "enum": ["minimal", "inherited_summary", "full_fork", "isolated_worktree_fork"],
                    "description": "Optional context override for this sub-agent. Use isolated_worktree_fork when the child may edit files in its own git worktree."
                },
                "role": {
                    "type": "string",
                    "enum": ["default", "plan", "verification", "guide", "advisor", "fast", "teammate", "specialist", "dream_task"],
                    "description": "Agent role. Profiles may set this automatically.",
                    "default": "default"
                },
                "template": {
                    "type": "string",
                    "enum": ["explore", "verify", "plan", "general", "review", "debug"],
                    "description": "Built-in agent template: explore (code exploration), verify (verification), plan (task planning), general (general purpose), review (code review), debug (debugging)"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Operate on an existing agent by ID instead of creating a new one"
                },
                "action": {
                    "type": "string",
                    "enum": ["list", "resume", "read", "cancel"],
                    "description": "Action for agent lifecycle/progress. list shows active and durable sub-agent progress; resume reads the in-memory result if available; read loads durable task/artifact state; cancel stops a running sub-agent and marks durable state cancelled.",
                    "default": "resume"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum durable task states to return for action=list",
                    "default": 20,
                    "minimum": 1
                },
                "subtasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": { "type": "string" },
                            "prompt": { "type": "string" },
                            "files": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["description", "prompt"]
                    },
                    "description": "Multiple subtasks to execute in parallel. Each becomes an independent sub-agent."
                },
                "parallel": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true and subtasks are provided, execute them in parallel"
                },
                "memory_key": {
                    "type": "string",
                    "description": "Save agent result to memory with this key"
                },
                "memory_snapshot": {
                    "type": "boolean",
                    "default": false,
                    "description": "Create a memory snapshot before agent execution"
                },
                "fork_branches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "branch_name": { "type": "string" },
                            "prompt": { "type": "string" },
                            "description": { "type": "string" }
                        },
                        "required": ["branch_name", "prompt"]
                    },
                    "description": "Multiple branches to explore in parallel when forking. Parent memory is inherited."
                }
            },
            "required": []
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let desc = params["description"].as_str().unwrap_or("");
        let role = params["role"].as_str().unwrap_or("default");
        let template = params["template"].as_str().unwrap_or("");
        let context_mode = params["context_mode"].as_str().unwrap_or("");
        let prompt = params["prompt"].as_str().unwrap_or("");
        let prompt_summary = if prompt.len() > 60 {
            format!("{}...", &prompt[..60])
        } else {
            prompt.to_string()
        };
        format!(
            "agent: role={} template={} context_mode={} desc='{}' prompt='{}'",
            role, template, context_mode, desc, prompt_summary
        )
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("delegate subagent fork worktree")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        params["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        if params["agent_id"].is_null() && params["action"].as_str() == Some("list") {
            let limit = params["limit"].as_i64().unwrap_or(20).clamp(1, 100);
            return list_agent_progress(&context, context.agent_manager.as_ref(), limit).await;
        }
        if let Some(agent_id_str) = params["agent_id"].as_str() {
            if params["fork_branches"].is_null()
                && params["subtasks"].is_null()
                && params["description"].is_null()
            {
                match params["action"].as_str() {
                    Some("read") => return read_durable_agent_state(&context, agent_id_str),
                    Some("list") => {
                        let limit = params["limit"].as_i64().unwrap_or(20).clamp(1, 100);
                        return list_agent_progress(
                            &context,
                            context.agent_manager.as_ref(),
                            limit,
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        }

        let agent_manager = match &context.agent_manager {
            Some(manager) => manager.clone(),
            None => {
                return ToolResult::error(
                    "AgentManager not available. Sub-agent creation requires AgentManager to be configured."
                );
            }
        };

        // 1. Resume existing agent
        if let Some(agent_id_str) = params["agent_id"].as_str() {
            // Only resume if no other action specified
            if params["fork_branches"].is_null()
                && params["subtasks"].is_null()
                && params["description"].is_null()
            {
                return match params["action"].as_str().unwrap_or("resume") {
                    "list" => list_agent_progress(&context, Some(&agent_manager), 20).await,
                    "resume" => handle_resume(&agent_manager, agent_id_str).await,
                    "read" => read_durable_agent_state(&context, agent_id_str),
                    "cancel" => handle_cancel(&agent_manager, &context, agent_id_str).await,
                    other => ToolResult::error(format!(
                        "Invalid agent action '{}'. Expected list, resume, read, or cancel.",
                        other
                    )),
                };
            }
        } else if let Some(action) = params["action"].as_str() {
            if action != "resume" && action != "list" {
                return ToolResult::error("agent_id is required for agent action");
            }
        }

        let requested_timeout_secs = params["timeout_secs"].as_u64();
        let max_turns = params["max_turns"].as_u64().unwrap_or(10) as usize;
        let max_cost_usd = params["max_cost_usd"].as_f64();
        let profile = params["profile"]
            .as_str()
            .and_then(|name| crate::agent::profiles::find_profile(&context.working_dir, name));
        let mut definition = profile.as_ref().map(AgentDefinition::from_profile);
        let timeout_secs = requested_timeout_secs
            .or_else(|| profile.as_ref().and_then(|profile| profile.timeout_secs))
            .unwrap_or(300);
        let role = params["role"]
            .as_str()
            .and_then(AgentRole::parse)
            .or_else(|| profile.as_ref().map(|profile| profile.role))
            .unwrap_or_default();
        let template = params["template"]
            .as_str()
            .and_then(AgentTemplate::from_str);
        let context_mode_override = match params["context_mode"].as_str() {
            Some(value) => match AgentContextMode::parse(value) {
                Some(mode) => Some(mode),
                None => {
                    return ToolResult::error(format!(
                        "Invalid context_mode '{}'. Expected one of: minimal, inherited_summary, full_fork, isolated_worktree_fork",
                        value
                    ));
                }
            },
            None => None,
        };
        let requested_tools: Vec<String> = params["allowed_tools"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let max_turns = profile
            .as_ref()
            .and_then(|profile| profile.max_turns)
            .unwrap_or(max_turns);
        let max_cost_usd = max_cost_usd.or_else(|| profile.as_ref().and_then(|p| p.max_cost_usd));
        let allowed_tools = resolve_subagent_allowed_tools(
            requested_tools,
            profile.as_ref(),
            definition.as_ref(),
            role,
            template,
        );
        if let Some(definition) = definition.as_mut() {
            definition.role = role;
            definition.tools = allowed_tools.clone();
            definition.max_turns = max_turns;
            definition.timeout_secs = timeout_secs;
            if let Some(context_mode) = context_mode_override {
                definition.context_mode = context_mode;
            }
        }

        let ctx = ExecuteParams {
            agent_manager: &agent_manager,
            params: &params,
            context: &context,
            timeout_secs,
            max_turns,
            max_cost_usd,
            allowed_tools,
            role,
            template,
            definition,
            context_mode_override,
        };

        // 2. Fork branches with memory inheritance
        if params["fork_branches"].is_array() {
            return handle_fork_branches(ctx).await;
        }

        // 3. Parallel subtasks
        if params["subtasks"].is_array() {
            return handle_subtasks(ctx).await;
        }

        // 4. Single agent
        handle_single_agent(ctx).await
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        if params["agent_id"].is_null() && params["action"].as_str() == Some("list") {
            return false;
        }
        true
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if params["agent_id"].is_null() && params["action"].as_str() == Some("list") {
            return None;
        }
        if let Some(agent_id) = params["agent_id"].as_str() {
            return Some(match params["action"].as_str().unwrap_or("resume") {
                "cancel" => format!("Cancel running sub-agent {}?", agent_id),
                "read" => format!("Read durable state for sub-agent {}?", agent_id),
                _ => "Resume an existing sub-agent?".to_string(),
            });
        }
        if let Some(subtasks) = params["subtasks"].as_array() {
            return Some(format!(
                "This will spawn {} parallel sub-agents. Continue?",
                subtasks.len()
            ));
        }
        if let Some(branches) = params["fork_branches"].as_array() {
            return Some(format!(
                "This will fork {} exploration branches. Continue?",
                branches.len()
            ));
        }
        let desc = params["description"].as_str().unwrap_or("(no description)");
        let role = params["role"].as_str().unwrap_or("default");
        let isolated_notice = params["context_mode"]
            .as_str()
            .and_then(AgentContextMode::parse)
            .filter(|mode| mode.requires_isolated_worktree())
            .map(|_| " in an isolated worktree")
            .unwrap_or("");
        Some(format!(
            "This will create a '{}' sub-agent{} to handle:\n{}\n\nContinue?",
            role, isolated_notice, desc
        ))
    }

    fn is_available(&self, context: &ToolContext) -> bool {
        context.agent_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("Agent manager not configured".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn agent_tool_contract_discourages_blocking_delegation() {
        let tool = AgentTool;
        assert!(tool.description().contains("independent, parallel"));
        assert!(tool.description().contains("blocks the current next step"));
        assert!(tool
            .description()
            .contains("explorer, verifier, planner, implementer"));
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
    }

    #[test]
    fn default_subagent_tool_surfaces_are_role_scoped() {
        let explorer =
            default_subagent_allowed_tools(AgentRole::Default, Some(AgentTemplate::Explore));
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
    fn agent_tool_schema_exposes_lifecycle_actions() {
        let tool = AgentTool;
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
            .create_session("s1", "agent list test", "test-model")
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

        let result = AgentTool
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
            .create_session("s1", "agent read test", "test-model")
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

        let result = AgentTool
            .execute(
                json!({"agent_id": "agent_1", "action": "read"}),
                ToolContext::new(".", "s1").with_session_store(store),
            )
            .await;

        assert!(result.success, "read failed: {:?}", result.error);
        assert!(result.content.contains("Sub-agent agent_1"));
        assert_eq!(result.data.unwrap()["status"], "completed");
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
        let tool = AgentTool;
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
        let tool = AgentTool;
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
        let tool = AgentTool;
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
        let tool = AgentTool;
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
}

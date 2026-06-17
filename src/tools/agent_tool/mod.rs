//! Agent 工具 - 创建子 Agent
//!
//! 用于创建并委派任务给子 Agent

use crate::agent::agent::AgentConfig;
use crate::agent::envelope::{AgentTaskEnvelope, AgentTaskPriority};
use crate::agent::manager::AgentResult as ManagerAgentResult;
use crate::agent::profiles::{AgentContextMode, AgentDefinition, AgentProfile};
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType, AgentStatus};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

mod description;
mod result_state;
use result_state::*;

const SUBAGENT_CLAIM_PROOF_KIND: &str = "subagent_claim_only";
const PARENT_VERIFIED_SUBAGENT_PROOF_KIND: &str = "parent_verified_subagent_result";

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
pub struct AgentTool {
    description: String,
}

impl Default for AgentTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentTool {
    pub fn new() -> Self {
        Self {
            description: description::build_tool_description(std::path::Path::new(".")),
        }
    }

    pub fn with_working_dir(working_dir: &std::path::Path) -> Self {
        Self {
            description: description::build_tool_description(working_dir),
        }
    }
}

/// 加载文件上下文
async fn load_file_context(files: &[String], working_dir: &Path) -> String {
    let mut context = String::new();
    for file in files {
        let path = match crate::tools::file_tool::resolve_path(file, working_dir) {
            Ok(p) => p,
            Err(e) => {
                warn!("Access denied for context file '{}': {}", file, e);
                context.push_str(&format!("\n## File: {}\n(access denied: {})\n", file, e));
                continue;
            }
        };
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
            let allowed_tools = Vec::<String>::new();
            let mut data = json!({
                "agent_id": agent_id.to_string(),
                "status": status_str.to_lowercase(),
                "result": result.content.clone(),
                "resumed": true,
            });
            attach_subagent_proof_metadata(
                &mut data,
                &result,
                AgentRole::Default,
                None,
                &allowed_tools,
                false,
            );
            ToolResult::success_with_data(
                format!(
                    "Resumed agent {}\nStatus: {}\n\nResult:\n{}",
                    agent_id, status_str, result.content
                ),
                data,
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
    let manager_stats = match agent_manager {
        Some(manager) => Some(manager.stats().await),
        None => None,
    };

    let mut lines = vec![format!(
        "Sub-agent progress: {} durable task(s), {} active in-memory agent(s)",
        durable_states.len(),
        active_agents.len()
    )];
    if let Some(stats) = manager_stats.as_ref() {
        lines.push(format!(
            "Lifecycle cache: {} result(s), {} channel(s), {} completion waiter(s), {} terminal handle(s)",
            stats.cached_results,
            stats.message_channels,
            stats.completion_receivers,
            stats.terminal_agents
        ));
    }
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
            "manager_stats": manager_stats.map(|stats| json!({
                "active_agents": stats.active_agents,
                "terminal_agents": stats.terminal_agents,
                "message_channels": stats.message_channels,
                "completion_receivers": stats.completion_receivers,
                "cached_results": stats.cached_results,
            })),
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

                let mut branch_result = json!({
                    "branch_name": branch_name,
                    "agent_id": result.agent_id.to_string(),
                    "status": format!("{:?}", result.status).to_lowercase(),
                    "content": result.content.clone(),
                });
                attach_subagent_proof_metadata(
                    &mut branch_result,
                    &result,
                    ctx.role,
                    ctx.template,
                    &ctx.allowed_tools,
                    false,
                );
                branch_result["scope"] = json!("subagent_branch_result");
                results.push(branch_result);
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

    let mut data = json!({
        "description": description,
        "total_branches": branches_array.len(),
        "succeeded": success_count,
        "failed": branches_array.len() - success_count,
        "branches": results,
    });
    if let Some(first_branch) = data
        .get("branches")
        .and_then(serde_json::Value::as_array)
        .and_then(|branches| {
            branches
                .iter()
                .find(|branch| branch.get("content").is_some())
        })
        .cloned()
    {
        data["proof_kind"] = first_branch
            .get("proof_kind")
            .cloned()
            .unwrap_or_else(|| json!(SUBAGENT_CLAIM_PROOF_KIND));
        data["verification_proof_kind"] = first_branch
            .get("verification_proof_kind")
            .cloned()
            .unwrap_or_else(|| json!(SUBAGENT_CLAIM_PROOF_KIND));
        data["parent_verified"] = first_branch
            .get("parent_verified")
            .cloned()
            .unwrap_or(serde_json::Value::Bool(false));
        data["scope"] = json!("subagent_branch_result_set");
        data["residual_risk"] =
            json!("subagent branch outputs require parent runtime verification");
    }

    ToolResult::success_with_data(output, data)
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

    synthesize_results(
        description,
        results,
        &files,
        ctx.role,
        ctx.template,
        &ctx.allowed_tools,
    )
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

            let mut data = json!({
                "agent_id": result.agent_id.to_string(),
                "description": description,
                "status": status_str.to_lowercase(),
                "result": result.content.clone(),
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
                "allowed_tools": ctx.allowed_tools.clone(),
                "completed_at": result.completed_at.elapsed().as_secs()
            });
            attach_subagent_proof_metadata(
                &mut data,
                &result,
                ctx.role,
                ctx.template,
                &ctx.allowed_tools,
                false,
            );

            ToolResult::success_with_data(
                format!(
                    "Sub-agent {} completed with status: {}\n\nTask: {}{}\n\nResult:\n{}",
                    result.agent_id, status_str, description, files_info, result.content
                ),
                data,
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
        &self.description
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
        let profile = params["profile"].as_str().and_then(|name| {
            crate::agent::profiles::find_runnable_profile(&context.working_dir, name)
        });
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
mod tests;

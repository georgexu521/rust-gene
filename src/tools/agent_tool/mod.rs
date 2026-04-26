//! Agent 工具 - 创建子 Agent
//!
//! 用于创建并委派任务给子 Agent

use crate::agent::agent::AgentConfig;
use crate::agent::envelope::{AgentTaskEnvelope, AgentTaskPriority};
use crate::agent::manager::AgentResult as ManagerAgentResult;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tracing::{info, warn};

/// 子代理模板
#[derive(Debug, Clone, Copy)]
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
    profile: Option<&crate::agent::profiles::AgentProfile>,
    working_dir: &Path,
) -> anyhow::Result<ManagerAgentResult> {
    let file_context = load_file_context(files, working_dir).await;
    let mut system_prompt = build_system_prompt(template, role, description, prompt, &file_context);
    if let Some(profile) = profile {
        if !profile.system_prompt.trim().is_empty() {
            system_prompt = format!("{}\n\n{}", profile.system_prompt.trim(), system_prompt);
        }
    }

    let agent_config = AgentConfig::new(format!("sub-agent: {}", description))
        .with_description(description)
        .with_system_prompt(system_prompt)
        .with_max_turns(max_turns)
        .with_allowed_tools(allowed_tools.to_vec());
    let agent_config = if let Some(limit) = max_cost_usd {
        agent_config.with_max_cost_usd(limit)
    } else {
        agent_config
    }
    .with_role(role);

    info!("Spawning sub-agent for task: {}", description);

    let agent_id = agent_manager.spawn(agent_config, None).await?;
    info!("Sub-agent spawned: {}", agent_id);

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
    if let Some(profile) = profile {
        envelope.add_constraint(format!("profile={}", profile.name));
        if let Some(context_mode) = &profile.context {
            envelope.add_constraint(format!("context={}", context_mode));
        }
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

    agent_manager.wait_for_result(&agent_id, timeout_secs).await
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
    profile: Option<crate::agent::profiles::AgentProfile>,
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
            ctx.profile.as_ref(),
            &ctx.context.working_dir,
        )
        .await
        {
            Ok(result) => {
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
                    ctx.profile.as_ref(),
                    &ctx.context.working_dir,
                )
            })
            .collect();

        let completed = futures::future::join_all(futures).await;
        completed.into_iter().filter_map(|r| r.ok()).collect()
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
            ctx.profile.as_ref(),
            &ctx.context.working_dir,
        )
        .await
        {
            Ok(r) => vec![r],
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
        ctx.profile.as_ref(),
        &ctx.context.working_dir,
    )
    .await
    {
        Ok(result) => {
            let status_str = format!("{:?}", result.status);
            let files_info = if files.is_empty() {
                String::new()
            } else {
                format!("\nRelevant files: {}", files.join(", "))
            };

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
         Use this for parallel execution of independent tasks, \
         or when a task requires specialized context. \
         The sub-agent will work independently and report back when done. \
         Supports built-in templates (explore, verify, plan, review, debug, general), \
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
                                 Be specific about what needs to be done and expected output."
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
                    "description": "Optional tool whitelist for isolation. If set, sub-agent can only call these tools."
                },
                "profile": {
                    "type": "string",
                    "description": "Named agent profile such as explorer, verifier, or implementer. Project profiles can be defined in .priority-agent/agents/*.toml."
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
                    "description": "Resume an existing agent by ID instead of creating a new one"
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
        let prompt = params["prompt"].as_str().unwrap_or("");
        let prompt_summary = if prompt.len() > 60 {
            format!("{}...", &prompt[..60])
        } else {
            prompt.to_string()
        };
        format!(
            "agent: role={} template={} desc='{}' prompt='{}'",
            role, template, desc, prompt_summary
        )
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
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
                return handle_resume(&agent_manager, agent_id_str).await;
            }
        }

        let timeout_secs = params["timeout_secs"].as_u64().unwrap_or(300);
        let max_turns = params["max_turns"].as_u64().unwrap_or(10) as usize;
        let max_cost_usd = params["max_cost_usd"].as_f64();
        let profile = params["profile"]
            .as_str()
            .and_then(|name| crate::agent::profiles::find_profile(&context.working_dir, name));
        let mut allowed_tools: Vec<String> = params["allowed_tools"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if allowed_tools.is_empty() {
            if let Some(profile) = &profile {
                allowed_tools = profile.allowed_tools.clone();
            }
        }
        let role = params["role"]
            .as_str()
            .and_then(AgentRole::parse)
            .or_else(|| profile.as_ref().map(|profile| profile.role))
            .unwrap_or_default();
        let template = params["template"]
            .as_str()
            .and_then(AgentTemplate::from_str);
        let max_turns = profile
            .as_ref()
            .and_then(|profile| profile.max_turns)
            .unwrap_or(max_turns);
        let max_cost_usd = max_cost_usd.or_else(|| profile.as_ref().and_then(|p| p.max_cost_usd));

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
            profile,
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

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if params["agent_id"].as_str().is_some() {
            return Some("Resume an existing sub-agent?".to_string());
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
        Some(format!(
            "This will create a '{}' sub-agent to handle:\n{}\n\nContinue?",
            role, desc
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

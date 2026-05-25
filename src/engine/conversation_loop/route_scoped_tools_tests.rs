use super::tool_context_helpers::tool_allowed_by_context;
use super::ConversationLoop;
use crate::engine::intent_router::{IntentKind, IntentRouter, WorkflowKind};
use crate::services::api::Tool;
use crate::test_utils::env_guard::EnvVarGuard;
use std::collections::HashSet;

#[test]
fn allowed_tool_context_enforces_subagent_tool_scope() {
    assert!(tool_allowed_by_context(&None, "bash"));

    let allowed = Some(HashSet::from(["file_read".to_string(), "grep".to_string()]));
    assert!(tool_allowed_by_context(&allowed, "file_read"));
    assert!(tool_allowed_by_context(&allowed, "grep"));
    assert!(!tool_allowed_by_context(&allowed, "bash"));
}

fn fake_tools(names: &[&str]) -> Vec<Tool> {
    names
        .iter()
        .map(|name| Tool::new(*name, format!("{} tool", name)))
        .collect()
}

fn exposed_names(tools: &[Tool]) -> HashSet<String> {
    tools.iter().map(|tool| tool.name.clone()).collect()
}

fn sorted_tool_names(tools: &[Tool]) -> Vec<String> {
    let mut names = tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn runtime_diet_tool_universe() -> Vec<Tool> {
    fake_tools(&[
        "agent",
        "ask_user",
        "bash",
        "bash_cancel",
        "bash_output",
        "calculate",
        "datetime",
        "diff",
        "enter_plan_mode",
        "exit_plan_mode",
        "file_edit",
        "file_read",
        "file_write",
        "format",
        "git",
        "git_status",
        "git_diff",
        "glob",
        "grep",
        "install_dependencies",
        "json_query",
        "list_mcp_resources",
        "lsp",
        "mcp",
        "mcp_auth",
        "mcp_tool",
        "memory_load",
        "memory_save",
        "plan",
        "project_list",
        "read_mcp_resource",
        "refactor",
        "repl",
        "run_tests",
        "start_dev_server",
        "skill_manage",
        "skills_list",
        "skill_view",
        "swarm",
        "symbol_query",
        "task_create",
        "task_get",
        "task_list",
        "task_output",
        "task_stop",
        "task_update",
        "todo_write",
        "web_fetch",
        "web_search",
        "workbench",
        "worktree",
    ])
}

#[test]
fn route_scoped_tools_for_file_delete_keep_destructive_scope_small() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我把这个文件删了吧");
    let tools = fake_tools(&[
        "file_read",
        "file_write",
        "file_edit",
        "glob",
        "bash",
        "web_search",
        "memory_save",
        "mcp",
        "agent",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("glob"));
    assert!(exposed.contains("bash"));
    assert!(!exposed.contains("file_write"));
    assert!(!exposed.contains("file_edit"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_save"));
    assert!(!exposed.contains("mcp"));
    assert!(!exposed.contains("agent"));
}

#[test]
fn route_scoped_tools_for_local_inspection_prefer_structured_read_tools() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
    let tools = fake_tools(&[
        "file_read",
        "file_write",
        "file_edit",
        "glob",
        "bash",
        "web_search",
        "memory_save",
        "mcp",
        "agent",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("glob"));
    assert!(!exposed.contains("bash"));
    assert!(!exposed.contains("file_write"));
    assert!(!exposed.contains("file_edit"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_save"));
    assert!(!exposed.contains("mcp"));
    assert!(!exposed.contains("agent"));
}

#[test]
fn route_scoped_tools_for_terminal_operation_include_bash_without_write_tools() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route =
        IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");
    let tools = fake_tools(&[
        "file_read",
        "file_write",
        "file_edit",
        "glob",
        "bash",
        "web_search",
        "memory_save",
        "mcp",
        "agent",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("bash"));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("glob"));
    assert!(!exposed.contains("file_write"));
    assert!(!exposed.contains("file_edit"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_save"));
    assert!(!exposed.contains("mcp"));
    assert!(!exposed.contains("agent"));
}

#[test]
fn route_scoped_tools_for_python_creation_include_write_and_validation() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
    let tools = fake_tools(&[
        "project_list",
        "grep",
        "file_read",
        "file_write",
        "file_edit",
        "file_patch",
        "bash",
        "run_tests",
        "start_dev_server",
        "install_dependencies",
        "bash_output",
        "bash_cancel",
        "diff",
        "web_search",
        "git_status",
        "git_diff",
        "memory_save",
        "mcp",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("project_list"));
    assert!(exposed.contains("grep"));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("file_write"));
    assert!(exposed.contains("file_edit"));
    assert!(exposed.contains("file_patch"));
    assert!(exposed.contains("bash"));
    assert!(exposed.contains("run_tests"));
    assert!(exposed.contains("start_dev_server"));
    assert!(!exposed.contains("install_dependencies"));
    assert!(exposed.contains("bash_output"));
    assert!(exposed.contains("bash_cancel"));
    assert!(exposed.contains("diff"));
    assert!(exposed.contains("git_status"));
    assert!(exposed.contains("git_diff"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_save"));
    assert!(!exposed.contains("mcp"));
}

#[test]
fn route_scoped_tools_for_debugging_include_search_read_shell_and_edit() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("cargo test 报错了，帮我修一下");
    let tools = fake_tools(&[
        "project_list",
        "grep",
        "file_read",
        "file_write",
        "file_edit",
        "file_patch",
        "bash",
        "run_tests",
        "start_dev_server",
        "install_dependencies",
        "lsp",
        "symbol_query",
        "git_status",
        "git_diff",
        "web_search",
        "memory_load",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("grep"));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("file_write"));
    assert!(exposed.contains("file_edit"));
    assert!(exposed.contains("file_patch"));
    assert!(exposed.contains("bash"));
    assert!(exposed.contains("run_tests"));
    assert!(exposed.contains("start_dev_server"));
    assert!(!exposed.contains("install_dependencies"));
    assert!(exposed.contains("lsp"));
    assert!(exposed.contains("symbol_query"));
    assert!(exposed.contains("git_status"));
    assert!(exposed.contains("git_diff"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_load"));
}

#[test]
fn route_scoped_tools_for_dependency_install_intent_include_install_facade() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我安装项目依赖，package.json 已经在项目里");
    let tools = fake_tools(&[
        "project_list",
        "glob",
        "file_read",
        "bash",
        "install_dependencies",
        "run_tests",
        "web_search",
        "memory_save",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(route.dependency_install_intent);
    assert!(exposed.contains("bash"));
    assert!(exposed.contains("install_dependencies"));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("glob"));
    assert!(!exposed.contains("run_tests"));
    assert!(!exposed.contains("web_search"));
    assert!(!exposed.contains("memory_save"));
}

#[test]
fn route_scoped_tools_ignore_install_recommendation_without_install_intent() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let mut route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
    route
        .recommended_tools
        .push("install_dependencies".to_string());
    let tools = fake_tools(&["file_read", "bash", "install_dependencies"]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(!route.dependency_install_intent);
    assert!(exposed.contains("bash"));
    assert!(!exposed.contains("install_dependencies"));
}

#[test]
fn route_scoped_tools_for_generic_mcp_config_hide_auth_tool() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我看看 mcp 配置");
    let tools = fake_tools(&[
        "config",
        "mcp",
        "mcp_tool",
        "mcp_auth",
        "list_mcp_resources",
        "read_mcp_resource",
        "file_read",
        "bash",
        "ask_user",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(!route.mcp_auth_intent);
    assert!(exposed.contains("config"));
    assert!(exposed.contains("mcp"));
    assert!(exposed.contains("mcp_tool"));
    assert!(exposed.contains("list_mcp_resources"));
    assert!(exposed.contains("read_mcp_resource"));
    assert!(!exposed.contains("mcp_auth"));
}

#[test]
fn route_scoped_tools_for_config_audit_include_read_only_search() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = crate::engine::intent_router::IntentRoute {
        intent: crate::engine::intent_router::IntentKind::Configuration,
        confidence: 0.8,
        workflow: crate::engine::intent_router::WorkflowKind::Direct,
        retrieval: crate::engine::intent_router::RetrievalPolicy::Light,
        reasoning: crate::engine::intent_router::ReasoningPolicy::Medium,
        risk: crate::engine::intent_router::RiskLevel::Medium,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "read-only config audit needs local search".to_string(),
    };
    let tools = fake_tools(&["glob", "grep", "file_read", "bash", "file_edit", "ask_user"]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("glob"));
    assert!(exposed.contains("grep"));
    assert!(exposed.contains("file_read"));
    assert!(exposed.contains("bash"));
    assert!(!exposed.contains("file_edit"));
}

#[test]
fn route_scoped_tools_for_explicit_mcp_auth_include_auth_tool() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我给 mcp server 做 OAuth 授权登录");
    let tools = fake_tools(&["config", "mcp", "mcp_auth", "file_read", "bash", "ask_user"]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(route.mcp_auth_intent);
    assert!(exposed.contains("config"));
    assert!(exposed.contains("mcp"));
    assert!(exposed.contains("mcp_auth"));
}

#[test]
fn route_scoped_tools_hide_skill_tools_without_skill_relevance() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
    let tools = fake_tools(&[
        "project_list",
        "grep",
        "file_read",
        "file_write",
        "file_edit",
        "bash",
        "skills_list",
        "skill_view",
        "skill_manage",
    ]);

    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("file_write"));
    assert!(exposed.contains("file_edit"));
    assert!(!exposed.contains("skills_list"));
    assert!(!exposed.contains("skill_view"));
    assert!(!exposed.contains("skill_manage"));
}

#[test]
fn runtime_diet_sample_prompts_stay_within_route_tool_budgets() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    struct Sample {
        label: &'static str,
        prompt: &'static str,
        intent: IntentKind,
        workflow: WorkflowKind,
        max_tools: usize,
    }

    let samples = [
        Sample {
            label: "direct answer",
            prompt: "简单回答：2+2 等于几？",
            intent: IntentKind::DirectAnswer,
            workflow: WorkflowKind::Direct,
            max_tools: 0,
        },
        Sample {
            label: "scoped file delete",
            prompt: "帮我把这个文件删了吧",
            intent: IntentKind::DirectAnswer,
            workflow: WorkflowKind::Direct,
            max_tools: 4,
        },
        Sample {
            label: "local inspection",
            prompt: "请帮我看看桌面有没有 gex 文件夹",
            intent: IntentKind::DirectAnswer,
            workflow: WorkflowKind::Direct,
            max_tools: 4,
        },
        Sample {
            label: "terminal operation",
            prompt: "帮我看看默认 python 有没有安装 pygame，帮我安装一下吧",
            intent: IntentKind::DirectAnswer,
            workflow: WorkflowKind::Direct,
            max_tools: 5,
        },
        Sample {
            label: "python code creation",
            prompt: "帮我做一个贪吃蛇游戏吧，用 python 做吧",
            intent: IntentKind::CodeChange,
            workflow: WorkflowKind::CodeChange,
            max_tools: 19,
        },
        Sample {
            label: "running issue debug",
            prompt: "我在运行中发现了一个问题，你帮我看看是怎么回事吧",
            intent: IntentKind::Debugging,
            workflow: WorkflowKind::BugFix,
            max_tools: 19,
        },
        Sample {
            label: "reference comparison",
            prompt: "帮我对比 claude 和 opencode 的 agent 指令设计",
            intent: IntentKind::Research,
            workflow: WorkflowKind::Research,
            max_tools: 6,
        },
    ];

    let router = IntentRouter::new();
    let tools = runtime_diet_tool_universe();
    for sample in samples {
        let route = router.route(sample.prompt);
        assert_eq!(
            route.intent, sample.intent,
            "runtime diet sample '{}' routed to unexpected intent: {:?}; reason={}",
            sample.label, route.intent, route.reason
        );
        assert_eq!(
            route.workflow, sample.workflow,
            "runtime diet sample '{}' routed to unexpected workflow: {:?}; reason={}",
            sample.label, route.workflow, route.reason
        );

        let exposed = sorted_tool_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(
            exposed.len() <= sample.max_tools,
            "runtime diet sample '{}' exposed {} tools, budget {}; route={}; reason={}; exposed={:?}",
            sample.label,
            exposed.len(),
            sample.max_tools,
            route.compact_label(),
            route.reason,
            exposed
        );
    }
}

#[test]
fn route_scoped_tools_can_be_disabled_for_full_or_debug_exposure() {
    let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
    let tools = fake_tools(&[
        "file_read",
        "file_write",
        "bash",
        "web_search",
        "memory_save",
    ]);

    let mut env = EnvVarGuard::acquire_blocking();
    env.set("PRIORITY_AGENT_TOOL_PROFILE", "full");
    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("web_search"));
    assert!(exposed.contains("memory_save"));

    env.remove("PRIORITY_AGENT_TOOL_PROFILE");
    env.set("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE", "1");
    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("web_search"));
    assert!(exposed.contains("memory_save"));

    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.set("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS", "0");
    let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
    assert!(exposed.contains("web_search"));
    assert!(exposed.contains("memory_save"));
}

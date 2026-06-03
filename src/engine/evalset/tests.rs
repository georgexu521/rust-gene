use super::*;
use crate::engine::intent_router::{IntentKind, RetrievalPolicy, WorkflowKind};

#[test]
fn eval_runner_passes_matching_route() {
    let set = EvalSet {
        name: "smoke".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "debug-route".to_string(),
            prompt: "cargo test 报错了，帮我修复".to_string(),
            replay: EvalReplay::default(),
            expect: EvalExpect {
                intent: Some(IntentKind::Debugging),
                workflow: Some(WorkflowKind::BugFix),
                retrieval: Some(RetrievalPolicy::Project),
                recommended_tools: vec!["bash".to_string()],
                trace_events: vec!["prompt".to_string(), "intent".to_string()],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_reports_mismatch() {
    let set = EvalSet {
        name: "bad".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "bad-route".to_string(),
            prompt: "你好".to_string(),
            replay: EvalReplay::default(),
            expect: EvalExpect {
                intent: Some(IntentKind::Debugging),
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(!report.ok());
    assert_eq!(report.failed, 1);
    assert!(report.summary().contains("bad-route"));
}

#[test]
fn loads_yaml_evalset() {
    let yaml = r#"
name: route_smoke
scenarios:
  - id: memory
    prompt: "记住我喜欢 compact 状态栏"
    expect:
      intent: memory
      retrieval: memory
      recommended_tools: ["memory_save"]
"#;
    let set: EvalSet = serde_yaml::from_str(yaml).unwrap();
    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_tool_trajectory_and_reflection_gate() {
    let set = EvalSet {
        name: "trajectory".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "failed-edit".to_string(),
            prompt: "修改代码并修复测试".to_string(),
            replay: EvalReplay {
                tool_calls: vec![
                    EvalToolCall {
                        tool: "file_edit".to_string(),
                        success: true,
                        output: "edited src/main.rs".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "bash".to_string(),
                        success: false,
                        output: "cargo test failed".to_string(),
                        permission: None,
                    },
                ],
                verification_passed: Some(false),
                changed_files: vec!["src/main.rs".to_string()],
                ..Default::default()
            },
            expect: EvalExpect {
                tool_sequence: vec!["file_edit".to_string(), "bash".to_string()],
                failed_tool: Some("bash".to_string()),
                verification_passed: Some(false),
                reflection_status: Some("Blocked".to_string()),
                repair_required: Some(true),
                trace_events: vec![
                    "tool.start".to_string(),
                    "tool.done".to_string(),
                    "verify.done".to_string(),
                    "reflection.pass".to_string(),
                ],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_workflow_contract_events() {
    let set = EvalSet {
        name: "workflow_contract".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "contract-visible".to_string(),
            prompt: "帮我修改代码，新增标签过滤页面".to_string(),
            replay: EvalReplay {
                workflow_judgment: true,
                acceptance_review_accepted: Some(true),
                verification_passed: Some(true),
                changed_files: vec!["src/app.rs".to_string()],
                ..Default::default()
            },
            expect: EvalExpect {
                workflow: Some(WorkflowKind::CodeChange),
                trace_events: vec![
                    "workflow.judgment".to_string(),
                    "workflow.plan".to_string(),
                    "acceptance.review".to_string(),
                ],
                verification_passed: Some(true),
                repair_required: Some(false),
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_guided_debugging_event() {
    let set = EvalSet {
        name: "guided_debugging".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "tool-failure-debugging".to_string(),
            prompt: "cargo test 报错了，帮我修复".to_string(),
            replay: EvalReplay {
                tool_calls: vec![EvalToolCall {
                    tool: "bash".to_string(),
                    success: false,
                    output: "cargo test failed".to_string(),
                    permission: None,
                }],
                guided_debugging: true,
                ..Default::default()
            },
            expect: EvalExpect {
                workflow: Some(WorkflowKind::BugFix),
                failed_tool: Some("bash".to_string()),
                trace_events: vec!["tool.done".to_string(), "guided.debug".to_string()],
                repair_required: Some(true),
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_permission_denial_and_recovery_plan() {
    let set = EvalSet {
        name: "permission_recovery".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "permission-denial-retry".to_string(),
            prompt: "危险命令被拒绝后改用安全路径继续".to_string(),
            replay: EvalReplay {
                tool_calls: vec![
                    EvalToolCall {
                        tool: "bash".to_string(),
                        success: false,
                        output: "Permission denied: 'bash' requires user confirmation.".to_string(),
                        permission: Some(EvalPermissionReplay {
                            prompt: "Allow bash rm -rf fixtures/tmp?".to_string(),
                            approved: false,
                            decision: Some("reject_once".to_string()),
                            ..Default::default()
                        }),
                    },
                    EvalToolCall {
                        tool: "file_read".to_string(),
                        success: true,
                        output: "safe readonly fallback".to_string(),
                        permission: None,
                    },
                ],
                recovery_plans: vec![EvalRecoveryPlan {
                    source: "tool_execution".to_string(),
                    category: "permission_denied".to_string(),
                    action: "explain denial and retry with safe readonly path".to_string(),
                    retryable: false,
                    safe_retry: false,
                    suggested_command: Some("/permissions explain".to_string()),
                    status: "Planned".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            expect: EvalExpect {
                failed_tool: Some("bash".to_string()),
                tool_sequence: vec!["bash".to_string(), "file_read".to_string()],
                permission_approved: Some(false),
                permission_decision: Some("reject_once".to_string()),
                recovery_category: Some("permission_denied".to_string()),
                recovery_suggested_command: Some("/permissions explain".to_string()),
                recovery_safe_retry: Some(false),
                repair_required: Some(false),
                trace_events: vec![
                    "permission.request".to_string(),
                    "permission.resolve".to_string(),
                    "recovery.plan".to_string(),
                ],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_background_terminal_task() {
    let set = EvalSet {
        name: "background_terminal".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "bash-background-task".to_string(),
            prompt: "启动后台服务并读取一段输出".to_string(),
            replay: EvalReplay {
                tool_calls: vec![
                    EvalToolCall {
                        tool: "bash".to_string(),
                        success: true,
                        output: "Started background shell command. Handle: shell-bg-eval-1"
                            .to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "bash_output".to_string(),
                        success: true,
                        output: "server ready".to_string(),
                        permission: None,
                    },
                ],
                terminal_tasks: vec![EvalTerminalTaskReplay {
                    id: "shell-bg-eval-1".to_string(),
                    source_tool: "bash".to_string(),
                    status: "running".to_string(),
                    command: Some("npm run dev".to_string()),
                    handle: Some("shell-bg-eval-1".to_string()),
                    read_tool: Some("bash_output".to_string()),
                    cancel_tool: Some("bash_cancel".to_string()),
                    cancel_handle: Some("shell-bg-eval-1".to_string()),
                    output_path: None,
                    backgrounded: true,
                }],
                ..Default::default()
            },
            expect: EvalExpect {
                tool_sequence: vec!["bash".to_string(), "bash_output".to_string()],
                terminal_task_count: Some(1),
                terminal_task_id: Some("shell-bg-eval-1".to_string()),
                terminal_task_status: Some("running".to_string()),
                terminal_task_read_tool: Some("bash_output".to_string()),
                terminal_task_cancel_tool: Some("bash_cancel".to_string()),
                backgrounded_tool: Some("bash".to_string()),
                trace_events: vec!["tool.start".to_string(), "tool.done".to_string()],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_file_checkpoint_and_rewind() {
    let set = EvalSet {
        name: "file_rewind".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "file-edit-rewind".to_string(),
            prompt: "改一个文件，然后回滚这次修改".to_string(),
            replay: EvalReplay {
                tool_calls: vec![EvalToolCall {
                    tool: "file_edit".to_string(),
                    success: true,
                    output: "edited src/lib.rs with checkpoint cp_eval_1".to_string(),
                    permission: None,
                }],
                file_changes: vec![EvalFileChangeReplay {
                    id: "fc_eval_1".to_string(),
                    checkpoint_id: "cp_eval_1".to_string(),
                    path: "src/lib.rs".to_string(),
                    tool_name: "file_edit".to_string(),
                    existed_before: true,
                    before_hash: Some("before123".to_string()),
                    after_hash: Some("after456".to_string()),
                    diff: Some("-old\n+new".to_string()),
                    bytes_written: 42,
                }],
                rewind: Some(EvalRewindReplay {
                    target: "fc_eval_1".to_string(),
                    checkpoint_id: "cp_eval_1".to_string(),
                    command: "/rewind".to_string(),
                    restored_files: vec!["src/lib.rs".to_string()],
                    removed_files: Vec::new(),
                    failed_files: Vec::new(),
                }),
                ..Default::default()
            },
            expect: EvalExpect {
                tool_sequence: vec!["file_edit".to_string()],
                file_checkpoint_count: Some(1),
                file_change_id: Some("fc_eval_1".to_string()),
                file_checkpoint_id: Some("cp_eval_1".to_string()),
                file_checkpoint_path: Some("src/lib.rs".to_string()),
                rewind_target: Some("fc_eval_1".to_string()),
                rewind_command: Some("/rewind".to_string()),
                rewind_checkpoint_id: Some("cp_eval_1".to_string()),
                rewind_restored_files: Some(1),
                available_commands: vec!["/rewind".to_string(), "/checkpoints".to_string()],
                trace_events: vec!["tool.start".to_string(), "tool.done".to_string()],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_compaction_boundary_and_runtime_diet() {
    let set = EvalSet {
        name: "compaction_boundary".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "compaction-boundary".to_string(),
            prompt: "长会话压缩后继续执行当前修复".to_string(),
            replay: EvalReplay {
                context_compactions: vec![EvalContextCompactionReplay {
                    before_tokens: 122_000,
                    after_tokens: 64_000,
                    strategy: "semantic_boundary".to_string(),
                    boundary_id: Some("cb-eval-1".to_string()),
                    sequence: Some(7),
                    messages_before: Some(48),
                    messages_after: Some(19),
                    preserved_tail_count: Some(6),
                    provenance: vec![
                        "summary:project_state".to_string(),
                        "tail:latest_tool_results".to_string(),
                    ],
                }],
                runtime_diet: Some(EvalRuntimeDietReplay {
                    prompt_tokens: 64_000,
                    tool_schema_tokens: 3_200,
                    total_request_tokens: 67_200,
                    max_context_tokens: Some(128_000),
                    remaining_context_tokens: Some(60_800),
                    tool_result_chars: 4_096,
                    tool_result_tokens: 1_024,
                    truncated_tool_results: 1,
                    tool_result_artifacts: 1,
                    exposed_tools: 12,
                    memory_snapshot_chars: 0,
                    memory_snapshot_tokens: 0,
                    retrieval_items: 2,
                    retrieval_tokens: 720,
                    skill_list_chars: 0,
                    skill_list_tokens: 0,
                    route_scoped_tools: true,
                    workflow_context: "strict".to_string(),
                    closeout_visibility: "full".to_string(),
                    validation_evidence: "pending".to_string(),
                    warnings: Vec::new(),
                }),
                tool_calls: vec![EvalToolCall {
                    tool: "file_read".to_string(),
                    success: true,
                    output: "compacted context retained target".to_string(),
                    permission: None,
                }],
                ..Default::default()
            },
            expect: EvalExpect {
                context_compaction_count: Some(1),
                context_boundary_id: Some("cb-eval-1".to_string()),
                context_compaction_strategy: Some("semantic_boundary".to_string()),
                context_before_tokens: Some(122_000),
                context_after_tokens: Some(64_000),
                context_preserved_tail_count: Some(6),
                runtime_diet_total_request_tokens: Some(67_200),
                runtime_diet_remaining_context_tokens: Some(60_800),
                runtime_diet_route_scoped_tools: Some(true),
                runtime_diet_workflow_context: Some("strict".to_string()),
                tool_sequence: vec!["file_read".to_string()],
                trace_events: vec![
                    "context.compact".to_string(),
                    "runtime.diet".to_string(),
                    "tool.done".to_string(),
                ],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_subagent_isolated_worktree_review_merge_cleanup() {
    let set = EvalSet {
        name: "subagent_worktree".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "subagent-worktree-worker".to_string(),
            prompt: "派子 agent 在隔离 worktree 里修改路由，然后 review/merge/cleanup".to_string(),
            replay: EvalReplay {
                subagents: vec![EvalSubagentReplay {
                    agent_id: "agent_eval_1".to_string(),
                    profile: Some("implementer".to_string()),
                    role: "specialist".to_string(),
                    description: "Implement scoped route repair".to_string(),
                    timeout_secs: 300,
                    allowed_tools: 4,
                    status: "completed".to_string(),
                    duration_ms: 1_200,
                    output_chars: 512,
                    tools_used: 3,
                    context_mode: Some("isolated_worktree_fork".to_string()),
                    worktree_path: Some(
                        "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval".to_string(),
                    ),
                    worktree_branch: Some("codex/agent-eval1".to_string()),
                    recursive_fork_guard: true,
                    placeholder_complete: true,
                    fork_message_count: Some(4),
                    parent_tool_call_ids: vec!["parent_call_1".to_string()],
                    cleanup_hooks: vec!["worktree_cleanup".to_string()],
                }],
                agent_worktree_actions: vec![
                    EvalAgentWorktreeActionReplay {
                        action: "agent_review".to_string(),
                        agent_id: "agent_eval_1".to_string(),
                        command: Some("/agents worktree review agent_eval_1".to_string()),
                        status: "success".to_string(),
                        path: Some(
                            "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                .to_string(),
                        ),
                        branch: Some("codex/agent-eval1".to_string()),
                        commits_ahead: Some(1),
                        merge_kind: None,
                        cleanup: false,
                        delete_branch: false,
                    },
                    EvalAgentWorktreeActionReplay {
                        action: "agent_merge".to_string(),
                        agent_id: "agent_eval_1".to_string(),
                        command: Some("/agents worktree merge agent_eval_1 --yes".to_string()),
                        status: "success".to_string(),
                        path: Some(
                            "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                .to_string(),
                        ),
                        branch: Some("codex/agent-eval1".to_string()),
                        commits_ahead: Some(1),
                        merge_kind: Some("branch".to_string()),
                        cleanup: false,
                        delete_branch: false,
                    },
                    EvalAgentWorktreeActionReplay {
                        action: "agent_cleanup".to_string(),
                        agent_id: "agent_eval_1".to_string(),
                        command: Some(
                            "/agents worktree cleanup agent_eval_1 --yes --delete-branch"
                                .to_string(),
                        ),
                        status: "success".to_string(),
                        path: Some(
                            "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                .to_string(),
                        ),
                        branch: Some("codex/agent-eval1".to_string()),
                        commits_ahead: None,
                        merge_kind: None,
                        cleanup: true,
                        delete_branch: true,
                    },
                ],
                tool_calls: vec![
                    EvalToolCall {
                        tool: "agent".to_string(),
                        success: true,
                        output: "agent_eval_1 completed in isolated worktree".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "worktree".to_string(),
                        success: true,
                        output: "Agent worktree review: agent_eval_1".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "worktree".to_string(),
                        success: true,
                        output: "Merged branch: codex/agent-eval1".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "worktree".to_string(),
                        success: true,
                        output: "Removed agent worktree".to_string(),
                        permission: None,
                    },
                ],
                ..Default::default()
            },
            expect: EvalExpect {
                subagent_count: Some(1),
                subagent_agent_id: Some("agent_eval_1".to_string()),
                subagent_profile: Some("implementer".to_string()),
                subagent_role: Some("specialist".to_string()),
                subagent_status: Some("completed".to_string()),
                subagent_context_mode: Some("isolated_worktree_fork".to_string()),
                subagent_allowed_tools: Some(4),
                isolated_worktree_path: Some(
                    "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval".to_string(),
                ),
                isolated_worktree_branch: Some("codex/agent-eval1".to_string()),
                recursive_fork_guard: Some(true),
                fork_placeholder_complete: Some(true),
                fork_message_count: Some(4),
                agent_worktree_action_count: Some(3),
                agent_worktree_review_command: Some(
                    "/agents worktree review agent_eval_1".to_string(),
                ),
                agent_worktree_merge_command: Some(
                    "/agents worktree merge agent_eval_1 --yes".to_string(),
                ),
                agent_worktree_cleanup_command: Some(
                    "/agents worktree cleanup agent_eval_1 --yes --delete-branch".to_string(),
                ),
                agent_worktree_review_status: Some("success".to_string()),
                agent_worktree_merge_status: Some("success".to_string()),
                agent_worktree_cleanup_status: Some("success".to_string()),
                agent_worktree_merge_kind: Some("branch".to_string()),
                agent_worktree_cleanup_deleted_branch: Some(true),
                tool_sequence: vec![
                    "agent".to_string(),
                    "worktree".to_string(),
                    "worktree".to_string(),
                    "worktree".to_string(),
                ],
                trace_events: vec![
                    "subagent.start".to_string(),
                    "subagent.done".to_string(),
                    "tool.done".to_string(),
                ],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_runner_replays_mcp_auth_repair_and_retry() {
    let set = EvalSet {
        name: "mcp_auth_repair".to_string(),
        description: String::new(),
        scenarios: vec![EvalScenario {
            id: "mcp-auth-repair".to_string(),
            prompt: "MCP server 未批准时提示修复，然后批准并重试 resource read".to_string(),
            replay: EvalReplay {
                mcp_resources: vec![
                    EvalMcpResourceReplay {
                        server: "filesystem".to_string(),
                        uri: "file:///repo/README.md".to_string(),
                        action: "read".to_string(),
                        success: false,
                        content_chars: 0,
                    },
                    EvalMcpResourceReplay {
                        server: "filesystem".to_string(),
                        uri: "file:///repo/README.md".to_string(),
                        action: "read".to_string(),
                        success: true,
                        content_chars: 128,
                    },
                ],
                mcp_repairs: vec![EvalMcpRepairReplay {
                    server: "filesystem".to_string(),
                    category: "approval".to_string(),
                    command: "/mcp approve filesystem".to_string(),
                    panel_command: "/panel mcp".to_string(),
                    status: "Planned".to_string(),
                    safe_retry: false,
                }],
                tool_calls: vec![
                    EvalToolCall {
                        tool: "read_mcp_resource".to_string(),
                        success: false,
                        output: "MCP server 'filesystem' is pending approval".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "mcp".to_string(),
                        success: true,
                        output: "MCP server 'filesystem' approved.".to_string(),
                        permission: None,
                    },
                    EvalToolCall {
                        tool: "read_mcp_resource".to_string(),
                        success: true,
                        output: "resource content after approval".to_string(),
                        permission: None,
                    },
                ],
                ..Default::default()
            },
            expect: EvalExpect {
                failed_tool: Some("read_mcp_resource".to_string()),
                tool_sequence: vec![
                    "read_mcp_resource".to_string(),
                    "mcp".to_string(),
                    "read_mcp_resource".to_string(),
                ],
                mcp_resource_count: Some(2),
                mcp_resource_failure_count: Some(1),
                mcp_resource_success_count: Some(1),
                mcp_resource_server: Some("filesystem".to_string()),
                mcp_resource_uri: Some("file:///repo/README.md".to_string()),
                mcp_resource_action: Some("read".to_string()),
                mcp_resource_success: Some(true),
                mcp_resource_content_chars: Some(128),
                mcp_repair_count: Some(1),
                mcp_repair_server: Some("filesystem".to_string()),
                mcp_repair_category: Some("approval".to_string()),
                mcp_repair_command: Some("/mcp approve filesystem".to_string()),
                mcp_repair_status: Some("Planned".to_string()),
                mcp_panel_command: Some("/panel mcp".to_string()),
                recovery_category: Some("mcp_approval_required".to_string()),
                recovery_suggested_command: Some("/mcp approve filesystem".to_string()),
                recovery_safe_retry: Some(false),
                available_commands: vec!["/mcp".to_string(), "/panel".to_string()],
                trace_events: vec![
                    "mcp.resource".to_string(),
                    "recovery.plan".to_string(),
                    "tool.done".to_string(),
                ],
                ..Default::default()
            },
        }],
    };

    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn bundled_smoke_evalset_passes() {
    let path = std::path::Path::new("evalsets/smoke.yaml");
    if !path.exists() {
        return;
    }
    let set = load_evalset(path).unwrap();
    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn bundled_feature_reality_evalset_passes() {
    let path = std::path::Path::new("evalsets/feature_reality.yaml");
    if !path.exists() {
        return;
    }
    let set = load_evalset(path).unwrap();
    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn bundled_coding_replay_matrix_passes() {
    let path = std::path::Path::new("evalsets/coding_replay_matrix.yaml");
    if !path.exists() {
        return;
    }
    let set = load_evalset(path).unwrap();
    assert!(
        set.scenarios.len() >= 25,
        "coding replay matrix should cover at least 25 scenarios"
    );
    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn bundled_tool_file_reliability_gauntlet_passes() {
    let path = std::path::Path::new("evalsets/tool_file_reliability_gauntlet.yaml");
    if !path.exists() {
        return;
    }
    let set = load_evalset(path).unwrap();
    assert_eq!(
        set.scenarios.len(),
        12,
        "Track G minimum deterministic gauntlet should keep exactly 12 scenarios"
    );
    let report = EvalRunner::new().run_set(&set);
    assert!(report.ok(), "{}", report.summary());
}

#[test]
fn eval_reports_json_contains_trend_fields() {
    let reports = vec![EvalReport {
        set_name: "sample".to_string(),
        total: 2,
        passed: 1,
        failed: 1,
        failures: vec![EvalFailure {
            scenario_id: "case-1".to_string(),
            message: "expected trace event".to_string(),
        }],
    }];
    let json = format_reports_json(&reports).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["sets"], 1);
    assert_eq!(value["scenarios"], 2);
    assert_eq!(value["passed"], 1);
    assert_eq!(value["failed"], 1);
    assert!(value["generated_at"].as_str().unwrap_or("").contains('T'));
    assert_eq!(value["reports"][0]["failures"][0]["scenario_id"], "case-1");
}

#[test]
fn safe_eval_report_label_removes_path_separators() {
    assert_eq!(
        safe_eval_report_label("../coding replay/matrix.yaml"),
        "coding-replay-matrix-yaml"
    );
    assert_eq!(safe_eval_report_label("../../"), "all");
    assert_eq!(safe_eval_report_label("smoke_1"), "smoke_1");
}

#[test]
fn write_reports_json_creates_trend_file() {
    let dir = tempfile::tempdir().unwrap();
    let reports = vec![EvalReport {
        set_name: "sample".to_string(),
        total: 1,
        passed: 1,
        failed: 0,
        failures: Vec::new(),
    }];

    let path = write_reports_json(&reports, dir.path(), "../sample").unwrap();

    assert_eq!(path.parent(), Some(dir.path()));
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("-sample.json"));
    let json = fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["sets"], 1);
    assert_eq!(value["scenarios"], 1);
    assert_eq!(value["failed"], 0);
}

#[test]
fn load_eval_report_bundles_returns_latest_first() {
    let dir = tempfile::tempdir().unwrap();
    let old = EvalReportBundle {
        generated_at: "2026-05-03T01:00:00Z".to_string(),
        sets: 1,
        scenarios: 2,
        passed: 1,
        failed: 1,
        baseline: None,
        reports: Vec::new(),
    };
    let new = EvalReportBundle {
        generated_at: "2026-05-03T02:00:00Z".to_string(),
        sets: 1,
        scenarios: 2,
        passed: 2,
        failed: 0,
        baseline: None,
        reports: Vec::new(),
    };
    fs::write(
        dir.path().join("eval-20260503T010000Z-all.json"),
        serde_json::to_string_pretty(&old).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.path().join("eval-20260503T020000Z-all.json"),
        serde_json::to_string_pretty(&new).unwrap(),
    )
    .unwrap();
    fs::write(dir.path().join("notes.json"), "{}").unwrap();

    let entries = load_eval_report_bundles(dir.path(), 10).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].1.generated_at, "2026-05-03T02:00:00Z");
    assert_eq!(entries[1].1.generated_at, "2026-05-03T01:00:00Z");
}

#[test]
fn format_eval_trend_shows_latest_and_delta() {
    let entries = vec![
        (
            PathBuf::from("eval-20260503T020000Z-all.json"),
            EvalReportBundle {
                generated_at: "2026-05-03T02:00:00Z".to_string(),
                sets: 1,
                scenarios: 3,
                passed: 3,
                failed: 0,
                baseline: None,
                reports: Vec::new(),
            },
        ),
        (
            PathBuf::from("eval-20260503T010000Z-all.json"),
            EvalReportBundle {
                generated_at: "2026-05-03T01:00:00Z".to_string(),
                sets: 1,
                scenarios: 2,
                passed: 1,
                failed: 1,
                baseline: None,
                reports: Vec::new(),
            },
        ),
    ];

    let trend = format_eval_trend(&entries);

    assert!(trend.contains("Eval Trend"));
    assert!(trend.contains("Latest: eval-20260503T020000Z-all.json"));
    assert!(trend.contains("scenarios=+1"));
    assert!(trend.contains("passed=+2"));
    assert!(trend.contains("failed=-1"));
}

#[test]
fn eval_report_bundle_parses_legacy_json_without_baseline() {
    let json = r#"{
            "generated_at": "2026-05-03T01:00:00Z",
            "sets": 1,
            "scenarios": 2,
            "passed": 2,
            "failed": 0,
            "reports": []
        }"#;

    let bundle: EvalReportBundle = serde_json::from_str(json).unwrap();

    assert_eq!(bundle.generated_at, "2026-05-03T01:00:00Z");
    assert!(bundle.baseline.is_none());
}

#[test]
fn format_eval_trend_shows_external_baseline_delta() {
    let entries = vec![(
        PathBuf::from("eval-20260503T020000Z-all.json"),
        EvalReportBundle {
            generated_at: "2026-05-03T02:00:00Z".to_string(),
            sets: 1,
            scenarios: 20,
            passed: 18,
            failed: 2,
            baseline: Some(EvalBaselineSummary {
                name: "claude-code-local".to_string(),
                generated_at: Some("2026-05-03T01:30:00Z".to_string()),
                scenarios: 20,
                passed: 19,
                failed: 1,
            }),
            reports: Vec::new(),
        },
    )];

    let trend = format_eval_trend(&entries);

    assert!(trend.contains("Delta vs baseline 'claude-code-local'"));
    assert!(trend.contains("scenarios=+0"));
    assert!(trend.contains("passed=-1"));
    assert!(trend.contains("failed=+1"));
    assert!(trend.contains("baseline_generated=2026-05-03T01:30:00Z"));
    assert!(trend.contains("baseline=claude-code-local"));
}

#[test]
fn load_external_baselines_and_format_matrix_comparison() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("claude-code.yaml"),
        r#"
provider: claude-code
generated_at: "2026-05-21T12:00:00Z"
model: claude-opus
source: manual smoke run
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    tool_calls: 4
    repair_turns: 0
    evidence: "edited, tested, rewound"
  - id: bash_background_task
    outcome: fail
    validation_passed: false
    final_evidence_backed: true
    tool_calls: 3
    repair_turns: 1
  - id: extra_untracked_case
    outcome: pass
"#,
    )
    .unwrap();

    let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
    let rendered = format_external_baseline_comparison(&baselines, Some("all"));

    assert_eq!(baselines.len(), 1);
    assert!(rendered.contains("External Baseline Comparison"));
    assert!(rendered.contains("claude-code [claude-opus]"));
    assert!(rendered.contains("coverage=2/6 pass=1 fail=1 blocked=0 not_run=0"));
    assert!(rendered.contains("missing: permission_denial_retry"));
    assert!(rendered.contains("unknown: extra_untracked_case"));
    assert!(rendered.contains("- file_edit_rewind: pass validation=true"));
    assert!(rendered.contains("evidence=edited, tested, rewound"));
}

#[test]
fn external_baseline_provider_filter_reports_missing_provider() {
    let rendered = format_external_baseline_comparison(&[], Some("codex"));

    assert!(rendered.contains("No external baseline found for provider 'codex'"));
    assert!(rendered.contains("evalsets/external_baselines"));
}

#[test]
fn external_baseline_template_covers_required_phase_12_ids() {
    let yaml = format_external_baseline_template("codex", Some("gpt-5.2")).unwrap();
    let parsed: EvalExternalBaselineSet = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(parsed.provider, "codex");
    assert_eq!(parsed.model.as_deref(), Some("gpt-5.2"));
    assert_eq!(
        parsed.scenarios.len(),
        crate::engine::scenario_matrix::deterministic_scenarios().len()
    );
    assert!(parsed
        .scenarios
        .iter()
        .all(|scenario| scenario.outcome == EvalExternalBaselineOutcome::NotRun));
    assert!(parsed
        .scenarios
        .iter()
        .any(|scenario| scenario.id == "mcp_auth_repair"));
}

#[test]
fn write_external_baseline_template_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();

    let path = write_external_baseline_template(dir.path(), "claude-code", None).unwrap();
    let err = write_external_baseline_template(dir.path(), "claude-code", None).unwrap_err();

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("baseline-claude-code.yaml")
    );
    assert!(err.to_string().contains("refusing to overwrite"));
}

#[test]
fn imports_external_baseline_from_markdown_table() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("claude-run.md");
    fs::write(
        &artifact,
        r#"
| scenario | result | validation | evidence backed | tools | repairs | evidence |
| --- | --- | --- | --- | --- | --- | --- |
| file_edit_rewind | pass | yes | yes | 4 | 0 | checkpoint restored |
| bash_background_task | fail | no | yes | 3 | 1 | task timed out |
| unknown_case | pass | yes | yes | 1 | 0 | ignored |
"#,
    )
    .unwrap();

    let baseline =
        load_external_baseline_artifact(&artifact, "claude-code", Some("claude-opus")).unwrap();

    assert_eq!(baseline.provider, "claude-code");
    assert_eq!(baseline.model.as_deref(), Some("claude-opus"));
    assert_eq!(baseline.scenarios.len(), 2);
    let file_case = baseline
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "file_edit_rewind")
        .unwrap();
    assert_eq!(file_case.outcome, EvalExternalBaselineOutcome::Pass);
    assert_eq!(file_case.validation_passed, Some(true));
    assert_eq!(file_case.final_evidence_backed, Some(true));
    assert_eq!(file_case.tool_calls, Some(4));
    assert_eq!(file_case.repair_turns, Some(0));
    assert_eq!(file_case.evidence.as_deref(), Some("checkpoint restored"));
}

#[test]
fn write_external_baseline_import_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("codex-run.md");
    fs::write(
        &artifact,
        r#"
| id | outcome | evidence |
| --- | --- | --- |
| mcp_auth_repair | blocked | auth unavailable |
"#,
    )
    .unwrap();

    let path =
        write_external_baseline_import(&artifact, dir.path(), "codex", Some("gpt-5.2")).unwrap();
    let err = write_external_baseline_import(&artifact, dir.path(), "codex", Some("gpt-5.2"))
        .unwrap_err();

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("baseline-codex-import.yaml")
    );
    assert!(err.to_string().contains("refusing to overwrite"));
}

#[test]
fn validates_external_baseline_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baseline-claude-code.yaml");
    fs::write(
        &path,
        r#"
provider: claude-code
model: claude-opus
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: false
    evidence: "TODO: fill later"
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    evidence: "checkpoint restored"
  - id: bash_background_task
    outcome: fail
    evidence: "task timed out"
  - id: extra_case
    outcome: pass
"#,
    )
    .unwrap();

    let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
    let rendered = format_external_baseline_validation(&baselines, Some("all"));

    assert!(rendered.contains("External Baseline Validation"));
    assert!(rendered.contains("status=invalid"));
    assert!(rendered.contains("duplicate scenario file_edit_rewind appears 2 times"));
    assert!(rendered.contains("missing required scenario permission_denial_retry"));
    assert!(rendered.contains("unknown scenario extra_case"));
    assert!(rendered.contains("file_edit_rewind pass is missing final_evidence_backed=true"));
    assert!(rendered.contains("file_edit_rewind is missing concrete evidence"));
    assert!(rendered.contains("bash_background_task fail should record validation_passed=false"));
}

#[test]
fn validates_complete_external_baseline_as_valid() {
    let dir = tempfile::tempdir().unwrap();
    let mut baseline = external_baseline_template("codex", Some("gpt-5.2"));
    for scenario in &mut baseline.scenarios {
        scenario.outcome = EvalExternalBaselineOutcome::Pass;
        scenario.validation_passed = Some(true);
        scenario.final_evidence_backed = Some(true);
        scenario.evidence = Some(format!("artifact for {}", scenario.id));
    }
    fs::write(
        dir.path().join("baseline-codex.yaml"),
        serde_yaml::to_string(&baseline).unwrap(),
    )
    .unwrap();

    let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
    let rendered = format_external_baseline_validation(&baselines, Some("codex"));

    assert!(rendered.contains("status=valid coverage=6/6 errors=0 warnings=0"));
}

#[test]
fn formats_external_parity_report_with_provider_gaps() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("baseline-claude-code.yaml"),
        r#"
provider: claude-code
model: claude-opus
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    evidence: "checkpoint restored"
  - id: bash_background_task
    outcome: fail
    validation_passed: false
    final_evidence_backed: true
    evidence: "background handle lost"
  - id: permission_denial_retry
    outcome: pass
    validation_passed: true
    final_evidence_backed: false
    evidence: "manual transcript"
"#,
    )
    .unwrap();

    let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
    let rendered = format_external_parity_report(&baselines, Some("all"));

    assert!(rendered.contains("Phase 12 Parity Report"));
    assert!(rendered.contains("Local replay-ready: 6/6  External providers: 1"));
    assert!(rendered.contains("- claude-code [claude-opus]: pass=2 fail=1 blocked=0 not_run=3"));
    assert!(rendered.contains("file_edit_rewind [replay_fixture_ready]"));
    assert!(rendered.contains("claude-code=pass gap=none validation=true evidence_backed=true"));
    assert!(rendered.contains("claude-code=fail gap=external_failed validation=false"));
    assert!(rendered.contains("claude-code=pass gap=evidence_incomplete"));
    assert!(rendered.contains("claude-code=missing gap=external_missing"));
}

#[test]
fn parity_report_provider_filter_reports_missing_provider() {
    let rendered = format_external_parity_report(&[], Some("claude-code"));

    assert!(rendered.contains("Phase 12 Parity Report"));
    assert!(rendered.contains("No external baseline found for provider 'claude-code'"));
}

#[test]
fn writes_external_parity_report_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let mut baseline = external_baseline_template("codex", Some("gpt-5.2"));
    for scenario in &mut baseline.scenarios {
        scenario.outcome = EvalExternalBaselineOutcome::Pass;
        scenario.validation_passed = Some(true);
        scenario.final_evidence_backed = Some(true);
        scenario.evidence = Some(format!("artifact for {}", scenario.id));
    }
    let baseline_path = dir.path().join("baseline-codex.yaml");
    fs::write(&baseline_path, serde_yaml::to_string(&baseline).unwrap()).unwrap();

    let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
    let report_dir = dir.path().join("reports");
    let path = write_external_parity_report(&baselines, Some("codex"), &report_dir).unwrap();
    let content = fs::read_to_string(&path).unwrap();

    assert_eq!(path.parent(), Some(report_dir.as_path()));
    assert!(path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("parity-") && name.ends_with("-codex.txt")));
    assert!(content.contains("Phase 12 Parity Report"));
    assert!(content.contains("codex [gpt-5.2]: pass=6 fail=0 blocked=0 not_run=0"));
    assert!(content.contains("gap=none"));
}

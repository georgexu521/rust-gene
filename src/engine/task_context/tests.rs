use super::*;
use crate::engine::intent_router::IntentRouter;
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde_json::json;

#[test]
fn bundle_flags_missing_acceptance_for_code_change() {
    let route = IntentRouter::new().route("修改 CLI 状态栏");
    let bundle = TaskContextBundle::new("修改 CLI 状态栏", ".", route, None);
    assert!(bundle.needs_stronger_acceptance());
}

#[test]
fn bundle_deduplicates_context_lists() {
    let route = IntentRouter::new().route("你好");
    let mut bundle = TaskContextBundle::new("你好", ".", route, None);
    bundle.add_constraint("keep it short");
    bundle.add_constraint("keep it short");
    bundle.add_file("src/main.rs");
    bundle.add_file("src/main.rs");
    assert_eq!(bundle.constraints.len(), 1);
    assert_eq!(bundle.relevant_files.len(), 1);
    assert_eq!(bundle.agent_state.active_files.len(), 1);
}

#[test]
fn bundle_applies_model_workflow_judgment() {
    let route = IntentRouter::new().route("实现一个网站");
    let mut bundle = TaskContextBundle::new("实现一个网站", ".", route, None);
    let judgment = crate::engine::workflow_contract::ProgrammingWorkflowJudgment {
        task_type: "website".into(),
        complexity: crate::engine::workflow_contract::TaskComplexity::Medium,
        risk: crate::engine::intent_router::RiskLevel::Medium,
        requirement_complete_enough: true,
        needs_user_questions: false,
        question_reason: None,
        questions: Vec::new(),
        assumptions: vec!["Use local storage".into()],
        guided_reasoning_required: false,
        guided_reasoning_triggers: Vec::new(),
        plan: Vec::new(),
        acceptance: crate::engine::workflow_contract::AcceptanceContract::pending(
            "实现一个网站",
            vec!["Main page renders".into()],
            Vec::new(),
        ),
    };

    bundle.apply_workflow_judgment(judgment);

    assert!(bundle.workflow_judgment.is_some());
    assert!(bundle
        .constraints
        .iter()
        .any(|item| item.contains("Use local storage")));
    assert!(bundle
        .acceptance_checks
        .iter()
        .any(|item| item == "Main page renders"));
    assert!(bundle
        .agent_state
        .verification_plan
        .required_checks
        .iter()
        .any(|item| item == "Main page renders"));
    assert!(bundle
        .agent_state
        .risks
        .iter()
        .any(|item| item.contains("model-judged risk")));
    assert!(!bundle.needs_stronger_acceptance());
}

#[test]
fn direct_acceptance_checks_do_not_create_validation_requirements() {
    let route = IntentRouter::new().route("只读检查 src/engine/intent_router.rs，不要修改文件");
    let mut bundle = TaskContextBundle::new(
        "只读检查 src/engine/intent_router.rs，不要修改文件",
        ".",
        route,
        None,
    );

    bundle.add_acceptance_check("最终答案包含路由结论");

    assert!(bundle
        .acceptance_checks
        .iter()
        .any(|item| item == "最终答案包含路由结论"));
    assert!(bundle
        .agent_state
        .verification_plan
        .required_checks
        .is_empty());
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::NotRequired
    );
}

#[test]
fn agent_task_state_initializes_from_route_and_goal() {
    let route = IntentRouter::new().route("修复 src/main.rs 里的报错");
    let bundle = TaskContextBundle::new("修复 src/main.rs 里的报错", ".", route, None);

    assert_eq!(bundle.agent_state.mode, AgentTaskMode::Full);
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Understand);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Pending
    );
    assert!(bundle
        .agent_state
        .forbidden_actions
        .iter()
        .any(|item| item.contains("outside the requested scope")));
}

#[test]
fn agent_task_state_formats_context_zone_summary() {
    let route = IntentRouter::new().route("你好");
    let mut bundle = TaskContextBundle::new("你好", ".", route, None);
    bundle
        .agent_state
        .record_observation("test", "saw greeting");
    bundle.agent_state.mark_done("answered greeting");

    let rendered = bundle.agent_state.format_for_context_zone();

    assert!(rendered.contains("Goal: 你好"));
    assert!(rendered.contains("Mode: Direct"));
    assert!(rendered.contains("Mode score: Direct"));
    assert!(rendered.contains("Lightweight plan: none"));
    assert!(rendered.contains("Done: true"));
}

#[test]
fn tool_assisted_direct_tasks_get_light_mode_and_plan() {
    let route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
    let bundle = TaskContextBundle::new("请帮我看看桌面有没有 gex 文件夹", ".", route, None);

    assert_eq!(bundle.agent_state.mode, AgentTaskMode::Light);
    assert_eq!(bundle.agent_state.mode_score.mode, AgentTaskMode::Light);
    let plan = bundle
        .agent_state
        .lightweight_plan
        .as_ref()
        .expect("light plan");
    assert!(plan.heavy_contract_avoided);
    assert!(plan
        .steps
        .iter()
        .any(|step| step.action.contains("glob") || step.action.contains("file_read")));
    let rendered = bundle.agent_state.format_for_context_zone();
    assert!(rendered.contains("Mode: Light"));
    assert!(rendered.contains("Lightweight plan:"));
    assert!(rendered.contains("verification_required="));
}

#[test]
fn agent_task_state_records_bounded_observations_and_steps() {
    let route = IntentRouter::new().route("修改 src/main.rs");
    let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

    for index in 0..20 {
        bundle
            .agent_state
            .record_observation("test", format!("observation {index}"));
        bundle
            .agent_state
            .record_completed_step(AgentTaskStage::Understand, format!("step {index}"));
    }

    assert_eq!(bundle.agent_state.observations.len(), MAX_OBSERVATIONS);
    assert_eq!(
        bundle.agent_state.completed_steps.len(),
        MAX_COMPLETED_STEPS
    );
    assert_eq!(bundle.agent_state.observations[0].summary, "observation 8");
    assert_eq!(bundle.agent_state.completed_steps[0].summary, "step 8");
}

#[test]
fn agent_task_state_updates_from_tool_context_evidence() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    let edit_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: json!({"path": "src/lib.rs"}),
    };
    let edit_result = ToolResult::success_with_data(
        "edited",
        json!({
            "path": "src/lib.rs",
            "resolved_path": "/tmp/project/src/lib.rs",
            "replacements": 1,
            "bytes_written": 42,
            "diff": {
                "additions": 2,
                "deletions": 1,
                "changed_line_start": 4,
                "changed_line_end": 5,
                "unified_diff": "@@ -4 +4 @@\n-old\n+new\n"
            }
        }),
    );

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&edit_call, &edit_result);

    assert_eq!(observed, 1);
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Validate);
    assert!(bundle
        .agent_state
        .active_files
        .iter()
        .any(|path| path == &PathBuf::from("src/lib.rs")));
    assert!(bundle
        .agent_state
        .completed_steps
        .iter()
        .any(|step| { step.stage == AgentTaskStage::Edit && step.summary.contains("src/lib.rs") }));
    assert_eq!(bundle.agent_state.edit_snapshots.len(), 1);
    assert_eq!(
        bundle.agent_state.edit_snapshots[0].stage,
        AgentTaskStage::Validate
    );
    assert!(bundle.agent_state.edit_snapshots[0]
        .label
        .contains("edit succeeded"));

    let validation_call = ToolCall {
        id: "call_validation".to_string(),
        name: "bash".to_string(),
        arguments: json!({"command": "cargo test -q"}),
    };
    let validation_result = ToolResult::success_with_data(
        "ok",
        json!({
            "shell_result": {
                "command": "cargo test -q",
                "cwd": "/tmp/project",
                "exit_code": 0,
                "timed_out": false
            },
            "permission_request": {
                "id": "perm_1",
                "kind": "bash",
                "approved": true,
                "patterns": ["cargo test -q"],
                "allowed_always_rules": [],
                "metadata": {
                    "risk_level": "low",
                    "permission_decision": "allow_once"
                }
            }
        }),
    );

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&validation_call, &validation_result);

    assert_eq!(observed, 2);
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Closeout);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Verified
    );
    let rendered = bundle.agent_state.format_for_context_zone();
    assert!(rendered.contains("Recent steps:"));
    assert!(rendered.contains("validation passed: cargo test -q"));
    assert!(rendered.contains("Recent observations:"));
    assert!(rendered.contains("user approved bash"));
    assert!(rendered.contains("Recent edit snapshots:"));
    assert!(rendered.contains("edit succeeded"));
}

#[test]
fn agent_task_state_records_repair_snapshot_after_failed_validation() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    bundle.agent_state.add_active_file("src/lib.rs");
    bundle
        .agent_state
        .record_completed_step(AgentTaskStage::Edit, "file_edit changed src/lib.rs");
    bundle.agent_state.set_stage(AgentTaskStage::Validate);

    let validation_call = ToolCall {
        id: "call_validation".to_string(),
        name: "bash".to_string(),
        arguments: json!({"command": "cargo test -q"}),
    };
    let mut validation_result = ToolResult::error("tests failed");
    validation_result.data = Some(json!({
        "shell_result": {
            "command": "cargo test -q",
            "cwd": "/tmp/project",
            "exit_code": 101,
            "timed_out": false
        }
    }));

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&validation_call, &validation_result);

    assert_eq!(observed, 1);
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Failed
    );
    let snapshot = bundle
        .agent_state
        .edit_snapshots
        .last()
        .expect("failed validation should record repair snapshot");
    assert!(snapshot.label.contains("validation failed"));
    assert_eq!(snapshot.stage, AgentTaskStage::Repair);
    assert_eq!(snapshot.verification_status, VerificationStatus::Failed);
    assert!(snapshot
        .active_files
        .iter()
        .any(|path| path == &PathBuf::from("src/lib.rs")));
}

#[test]
fn agent_task_state_keeps_bounded_edit_snapshots() {
    let route = IntentRouter::new().route("修改 src/main.rs");
    let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

    for index in 0..10 {
        bundle
            .agent_state
            .record_edit_snapshot(format!("snapshot {index}"));
    }

    assert_eq!(bundle.agent_state.edit_snapshots.len(), MAX_EDIT_SNAPSHOTS);
    assert_eq!(bundle.agent_state.edit_snapshots[0].label, "snapshot 4");
}

#[test]
fn agent_task_state_records_tool_observation_metadata() {
    let route = IntentRouter::new().route("查看 src/lib.rs");
    let mut bundle = TaskContextBundle::new("查看 src/lib.rs", ".", route, None);
    let call = ToolCall {
        id: "call_read".to_string(),
        name: "file_read".to_string(),
        arguments: json!({"path": "src/lib.rs"}),
    };
    let result = ToolResult::success_with_data(
        "read file",
        json!({
            "tool_observation": {
                "schema": "tool_observation.v1",
                "tool": "file_read",
                "call_id": "call_read",
                "status": "success",
                "summary": "file_read succeeded: read src/lib.rs",
                "files_read": ["src/lib.rs"],
                "files_changed": [],
                "command_run": null,
                "validation_result": null,
                "permission_decision": null,
                "checkpoint_id": null,
                "artifact_path": null,
                "state_updates": ["files_read"],
                "recommended_next_action": null
            },
            "action_decision": {
                "action": {
                    "stage": "Understand"
                },
                "scores": {
                    "value": 7,
                    "risk": 1,
                    "uncertainty_reduction": 8,
                    "cost": 2,
                    "reversibility": 10,
                    "scope_fit": 9,
                    "action_score": 24
                },
                "score_computation": {
                    "formula_stage": "diagnosis",
                    "formula_version": "action_score.v1"
                }
            },
            "action_review": {
                "decision": "allow"
            }
        }),
    );

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&call, &result);

    assert_eq!(observed, 1);
    assert!(bundle
        .agent_state
        .active_files
        .contains(&PathBuf::from("src/lib.rs")));
    assert!(bundle
        .agent_state
        .observations
        .iter()
        .any(|observation| observation.source == "tool_observation"
            && observation.summary.contains("file_read success")));
    assert_eq!(bundle.agent_state.action_score_history.len(), 1);
    let score = &bundle.agent_state.action_score_history[0];
    assert_eq!(score.action_score, 24);
    assert_eq!(score.scope_fit, 9);
    assert_eq!(score.review_decision.as_deref(), Some("allow"));
}

#[test]
fn agent_task_state_records_structured_observer_findings() {
    let route = IntentRouter::new().route("修复 cargo test 失败");
    let mut bundle = TaskContextBundle::new("修复 cargo test 失败", ".", route, None);
    let call = ToolCall {
        id: "call_test".to_string(),
        name: "bash".to_string(),
        arguments: json!({"command": "cargo test -q"}),
    };
    let result = ToolResult::error_with_content(
        "cargo test failed",
        "test auth::login --- FAILED\nerror[E0425]: cannot find value `token`",
    );
    let mut result = result;
    result.data = Some(json!({
        "tool_observation": {
            "schema": "tool_observation.v1",
            "tool": "bash",
            "call_id": "call_test",
            "status": "failed",
            "result_kind": "validation",
            "summary": "Validation `cargo test -q` failed.",
            "key_findings": ["Failed tests: auth::login."],
            "evidence": [{"kind": "diagnostic", "text": "error[E0425]: cannot find value `token`"}],
            "impact_on_goal": "Narrows the next step to repairing the reported validation failure.",
            "next_attention": ["Rerun `cargo test -q` after the next patch."],
            "files_read": [],
            "files_changed": [],
            "command_run": "cargo test -q",
            "validation_result": "failed",
            "permission_decision": null,
            "checkpoint_id": null,
            "artifact_path": null,
            "state_updates": ["validation_result"],
            "recommended_next_action": null,
            "include_in_next_context": true,
            "store_in_state": true,
            "confidence": 90,
            "raw_result_ref": null,
            "hypothesis_updates": [{
                "hypothesis": "current implementation does not satisfy the latest validation",
                "confidence": 80,
                "evidence": ["error[E0425]"]
            }],
            "candidate_focus": ["src/auth/login.rs"],
            "reduced_uncertainty": true,
            "risk_note": null
        }
    }));

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&call, &result);

    assert_eq!(observed, 2);
    assert!(bundle
        .agent_state
        .key_findings
        .iter()
        .any(|finding| finding.summary.contains("auth::login")));
    assert!(bundle
        .agent_state
        .hypotheses
        .iter()
        .any(|hypothesis| hypothesis.hypothesis.contains("latest validation")));
    assert!(bundle
        .agent_state
        .candidate_focus
        .iter()
        .any(|focus| focus.target == "src/auth/login.rs"));
    let rendered = bundle.agent_state.format_for_context_zone();
    assert!(rendered.contains("Key findings:"));
    assert!(rendered.contains("Hypotheses:"));
    assert!(rendered.contains("Candidate focus:"));
}

#[test]
fn agent_task_state_marks_denied_confirmation_user_deferred() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    let call = ToolCall {
        id: "call_denied".to_string(),
        name: "bash".to_string(),
        arguments: json!({"command": "rm -rf target"}),
    };
    let result = ToolResult::error_with_content(
        "Permission denied",
        json!({
            "permission_request": {
                "id": "perm_1",
                "kind": "bash",
                "approved": false,
                "patterns": ["rm -rf target"],
                "allowed_always_rules": [],
                "metadata": {
                    "risk_level": "high",
                    "permission_decision": "deny"
                }
            }
        })
        .to_string(),
    );
    let mut result = result;
    result.data = Some(json!({
        "permission_request": {
            "id": "perm_1",
            "kind": "bash",
            "approved": false,
            "patterns": ["rm -rf target"],
            "allowed_always_rules": [],
            "metadata": {
                "risk_level": "high",
                "permission_decision": "deny"
            }
        }
    }));

    let observed = bundle
        .agent_state
        .observe_tool_context_evidence(&call, &result);

    assert_eq!(observed, 1);
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::UserDeferred
    );
    assert!(bundle
        .agent_state
        .observations
        .iter()
        .any(|observation| observation.summary.contains("user denied bash")));
}

#[test]
fn agent_task_state_advances_from_understand_to_edit_after_successful_inspection() {
    let route = IntentRouter::new().route("修改 src/main.rs");
    let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

    bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: true,
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            has_worktree_changes: false,
            has_successful_validation_commands: false,
            failed_tool_evidence_present: false,
        });

    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Edit);
    assert!(bundle
        .agent_state
        .completed_steps
        .iter()
        .any(|step| step.stage == AgentTaskStage::Understand));
}

#[test]
fn agent_task_state_advances_to_validate_after_write_and_closeout_after_validation() {
    let route = IntentRouter::new().route("修改 src/main.rs");
    let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

    bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: true,
            batch_has_unsuccessful_tools: false,
            used_write_tool: true,
            successful_write_tool: true,
            has_worktree_changes: true,
            has_successful_validation_commands: false,
            failed_tool_evidence_present: false,
        });
    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Validate);

    bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: true,
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            has_worktree_changes: true,
            has_successful_validation_commands: true,
            failed_tool_evidence_present: false,
        });

    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Closeout);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Verified
    );
}

#[test]
fn agent_task_state_moves_to_repair_after_failed_tool_round() {
    let route = IntentRouter::new().route("修改 src/main.rs");
    let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

    bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: false,
            batch_has_unsuccessful_tools: true,
            used_write_tool: false,
            successful_write_tool: false,
            has_worktree_changes: false,
            has_successful_validation_commands: false,
            failed_tool_evidence_present: true,
        });

    assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
    assert_eq!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Failed
    );
}

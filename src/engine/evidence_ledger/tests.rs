use super::*;

fn tool_call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: name.to_string(),
        arguments: args,
    }
}

#[test]
fn records_file_write_as_changed_file_fact() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call("file_write", serde_json::json!({"path": "src/app.py"})),
        &ToolResult::success("Wrote file"),
    );

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.changed_files, vec!["src/app.py".to_string()]);
    assert_eq!(snapshot.tool_execution_records, 1);
    assert_eq!(snapshot.file_facts, 1);
    assert_eq!(snapshot.command_facts, 0);
    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.file_evidence.len(), 1);
    assert_eq!(record.file_evidence[0].fact_index, 0);
    assert_eq!(record.file_evidence[0].path.as_deref(), Some("src/app.py"));
}

#[test]
fn records_tool_contract_semantics_from_summary() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "listed files",
        serde_json::json!({
            "tool_summary": {
                "operation_kind": "list",
                "read_only": true,
                "concurrency_safe": true,
                "destructive": false
            }
        }),
    );

    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "ls -la"})),
        &result,
    );

    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.operation_kind.as_deref(), Some("list"));
    assert_eq!(record.read_only, Some(true));
    assert_eq!(record.concurrency_safe, Some(true));
    assert_eq!(record.destructive, Some(false));
}

#[test]
fn records_file_read_fact_metadata_from_tool_result_data() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "   2 | beta\n   3 | gamma",
        serde_json::json!({
            "kind": "file",
            "line_start": 2,
            "line_end": 3,
            "total_lines": 3,
            "displayed_lines": 2,
            "truncated": true,
            "content_hash": "abc123",
            "display_format": "line_numbered_content"
        }),
    );

    ledger.record_tool_result(
        &tool_call("file_read", serde_json::json!({"path": "src/lib.rs"})),
        &result,
    );

    let fact = &ledger.file_facts[0];
    assert_eq!(fact.kind.as_deref(), Some("file"));
    assert_eq!(fact.line_start, Some(2));
    assert_eq!(fact.line_end, Some(3));
    assert_eq!(fact.total_lines, Some(3));
    assert_eq!(fact.displayed_lines, Some(2));
    assert_eq!(fact.truncated, Some(true));
    assert_eq!(fact.content_hash.as_deref(), Some("abc123"));
    assert!(fact.summary.contains("line_numbered_content"));
    let output = &ledger.tool_execution_records()[0].output;
    assert!(output.data_keys.iter().any(|key| key == "content_hash"));
    assert_eq!(
        output.display_format.as_deref(),
        Some("line_numbered_content")
    );
    assert_eq!(output.truncated, Some(true));
}

#[test]
fn records_file_edit_diagnostics_metadata_from_tool_result_data() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "File edited successfully: src/lib.rs (1 replacement(s))",
        serde_json::json!({
            "path": "src/lib.rs",
            "replacements": 1,
            "diagnostics": {
                "checked": true,
                "status": "diagnostics_found",
                "diagnostic_count": 2,
                "error_count": 1,
                "warning_count": 1,
                "first_error": {
                    "message": "type mismatch in return value",
                    "source": "rust-analyzer",
                    "code": "E0308",
                    "range": {
                        "start_line": 7
                    }
                }
            }
        }),
    );

    ledger.record_tool_result(
        &tool_call("file_edit", serde_json::json!({"path": "src/lib.rs"})),
        &result,
    );

    let fact = &ledger.file_facts[0];
    assert!(fact
        .summary
        .contains("lsp_diagnostics=status:diagnostics_found"));
    assert!(fact.summary.contains("errors:1"));
    assert!(fact.summary.contains("warnings:1"));
    assert!(fact.summary.contains("first_error:line:7"));
    assert!(fact.summary.contains("source:rust-analyzer"));
    assert!(fact.summary.contains("code:E0308"));
    let diagnostics = ledger.tool_execution_records()[0]
        .output
        .diagnostics
        .as_ref()
        .expect("diagnostics metadata should be recorded");
    assert_eq!(diagnostics.status.as_deref(), Some("diagnostics_found"));
    assert_eq!(diagnostics.diagnostic_count, Some(2));
    assert_eq!(diagnostics.first_error_line, Some(7));
}

#[test]
fn records_file_patch_files_as_changed_file_facts() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "Applied file_patch successfully: 2 operation(s), 2 file(s)",
        serde_json::json!({
            "files": [
                {
                    "path": "src/lib.rs",
                    "replacements": 1,
                    "bytes_written": 42,
                    "diff": {
                        "additions": 1,
                        "deletions": 1,
                        "changed_line_start": 3,
                        "changed_line_end": 3,
                        "preview_truncated": false
                    }
                },
                {
                    "path": "README.md",
                    "replacements": 1,
                    "bytes_written": 20,
                    "diff": {
                        "additions": 2,
                        "deletions": 1,
                        "changed_line_start": 7,
                        "changed_line_end": 8,
                        "preview_truncated": false
                    }
                }
            ]
        }),
    );

    ledger.record_tool_result(
        &tool_call(
            "file_patch",
            serde_json::json!({
                "operations": [
                    {"path": "src/lib.rs"},
                    {"path": "README.md"}
                ]
            }),
        ),
        &result,
    );

    let snapshot = ledger.snapshot();
    assert_eq!(
        snapshot.changed_files,
        vec!["README.md".to_string(), "src/lib.rs".to_string()]
    );
    assert_eq!(snapshot.file_facts, 2);
    let fact = &ledger.file_facts[0];
    assert_eq!(fact.tool, "file_patch");
    assert_eq!(fact.path.as_deref(), Some("src/lib.rs"));
    assert_eq!(fact.kind.as_deref(), Some("patch"));
    assert_eq!(fact.line_start, Some(3));
    assert_eq!(fact.line_end, Some(3));
    assert_eq!(fact.truncated, Some(false));
    assert!(fact.summary.contains("bytes_written=42"));
    assert!(fact.summary.contains("changed_line_start=3"));
    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.file_evidence.len(), 2);
    assert_eq!(record.file_evidence[0].fact_index, 0);
    assert_eq!(record.file_evidence[0].path.as_deref(), Some("src/lib.rs"));
    assert_eq!(record.file_evidence[0].line_start, Some(3));
    assert_eq!(record.file_evidence[1].fact_index, 1);
    assert_eq!(record.file_evidence[1].path.as_deref(), Some("README.md"));
    assert_eq!(record.file_evidence[1].line_end, Some(8));
    assert_eq!(record.output.file_count, Some(2));
}

#[test]
fn records_safe_bash_validation_as_command_and_validation_fact() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "cargo test -q src"})),
        &ToolResult::success("test result: ok"),
    );

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.command_facts, 1);
    assert_eq!(snapshot.validation_facts, 1);
    assert_eq!(snapshot.passed_validation_facts, 1);
    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("passed:1/1")
    );
    assert_eq!(
        ledger.command_facts[0].normalized_command,
        "cargo test -q src"
    );
    assert_eq!(ledger.command_facts[0].path_patterns, vec!["src"]);
    assert!(!ledger.command_facts[0].network_access);
    assert!(!ledger.command_facts[0].external_path_access);
    assert!(!ledger.command_facts[0].compound_command);
}

#[test]
fn records_shell_risk_facts_from_tool_summary() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "ok",
        serde_json::json!({
            "tool_summary": {
                "command": "curl https://example.com -o /tmp/out.json",
                "network_access": true,
                "external_path_access": true,
                "absolute_path_patterns": ["/tmp/out.json"],
                "compound_command": false,
                "shell_control_operators": [],
                "risky_shell_wrapper": false,
                "expected_silent_output": false,
                "permission_rule_suggestions": [
                    {
                        "pattern": "curl https://example.com -o /tmp/out.json",
                        "scope": "exact",
                        "stable": false,
                        "reason": "exact command for this permission review"
                    }
                ]
            }
        }),
    );

    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({"command": "curl https://example.com -o /tmp/out.json"}),
        ),
        &result,
    );

    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.network_access, Some(true));
    assert_eq!(record.external_path_access, Some(true));
    assert_eq!(record.absolute_path_patterns, vec!["/tmp/out.json"]);
    assert_eq!(record.compound_command, Some(false));
    assert_eq!(record.risky_shell_wrapper, Some(false));
    assert_eq!(record.expected_silent_output, Some(false));
    assert_eq!(record.permission_rule_suggestions.len(), 1);
}

#[test]
fn records_bash_mutation_path_patterns_as_changed_paths() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "git add src/lib.rs"})),
        &ToolResult::success(""),
    );

    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.changed_paths, vec!["src/lib.rs"]);
    assert!(record.relevance.closeout);
    assert!(record.relevance.repair);
}

#[test]
fn records_tool_execution_record_with_command_and_terminal_metadata() {
    let mut ledger = EvidenceLedger::new();
    let mut result = ToolResult::success_with_data(
        "test result: ok",
        serde_json::json!({
            "terminal_task": {
                "task_id": "shell_foreground_123",
                "status": "completed",
                "terminal_kind": "foreground_shell",
                "duration_ms": 42,
                "exit_code": 0
            },
            "tool_summary": {
                "tool": "bash",
                "call_id": "call_1",
                "success": true,
                "duration_ms": 42,
                "output_chars": 15,
                "command": "cargo test -q",
                "command_kind": "validation",
                "command_category": "test_run",
                "validation_family": "cargo_test",
                "path_patterns": ["src/lib.rs"],
                "safe_for_closeout": true,
                "operation_kind": "shell",
                "read_only": false,
                "concurrency_safe": false,
                "destructive": false,
                "aliases": ["shell"],
                "search_hint": "shell validation git package managers",
                "should_defer": false,
                "always_load": false,
                "strict_schema": true,
                "interrupt_behavior": "block",
                "requires_user_interaction": false,
                "open_world": false,
                "search_or_read": {
                    "is_search": false,
                    "is_read": false,
                    "is_list": false
                },
                "input_paths": ["src/lib.rs"],
                "permission_matcher_input": "cargo test -q",
                "transcript_summary": "cargo test -q",
                "ui_render_kind": "shell",
                "terminal_task": {
                    "task_id": "shell_foreground_123",
                    "status": "completed",
                    "terminal_kind": "foreground_shell",
                    "duration_ms": 42,
                    "exit_code": 0
                }
            },
            "tool_runtime": {
                "route": {
                    "intent": "code_change",
                    "workflow": "code_change",
                    "retrieval": "project",
                    "reasoning": "high",
                    "risk": "medium"
                },
                "policy": {
                    "latency": "deep",
                    "parallelism_limit": 4,
                    "max_tool_calls": 30,
                    "context_budget_tokens": 64000,
                    "allow_fallback_model": true,
                    "cost_ceiling_usd": "0.2500"
                },
                "execution": {
                    "parallel": false,
                    "pre_executed": false,
                    "action_checkpoint_active": true,
                    "has_changes_before_tools": true,
                    "exposed_tools_count": 15,
                    "started_at_unix_ms": 1770000000000u64,
                    "finished_at_unix_ms": 1770000000042u64
                }
            }
        }),
    );
    result.duration_ms = Some(42);

    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
        &result,
    );

    let records = ledger.tool_execution_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tool, "bash");
    assert_eq!(records[0].status, ToolExecutionStatus::Completed);
    assert_eq!(records[0].arguments_hash.len(), HASH_PREVIEW_CHARS);
    assert_eq!(records[0].command.as_deref(), Some("cargo test -q"));
    assert_eq!(
        records[0].normalized_command.as_deref(),
        Some("cargo test -q")
    );
    assert_eq!(records[0].command_kind.as_deref(), Some("validation"));
    assert_eq!(records[0].validation_family.as_deref(), Some("cargo_test"));
    assert_eq!(records[0].path_patterns, vec!["src/lib.rs"]);
    assert_eq!(records[0].safe_for_closeout, Some(true));
    assert_eq!(records[0].operation_kind.as_deref(), Some("shell"));
    assert_eq!(records[0].read_only, Some(false));
    assert_eq!(records[0].concurrency_safe, Some(false));
    assert_eq!(records[0].destructive, Some(false));
    assert_eq!(records[0].aliases, vec!["shell"]);
    assert_eq!(
        records[0].search_hint.as_deref(),
        Some("shell validation git package managers")
    );
    assert_eq!(records[0].strict_schema, Some(true));
    assert_eq!(records[0].interrupt_behavior.as_deref(), Some("block"));
    assert_eq!(records[0].requires_user_interaction, Some(false));
    assert_eq!(records[0].open_world, Some(false));
    assert!(!records[0].search_or_read.is_search);
    assert_eq!(records[0].input_paths, vec!["src/lib.rs"]);
    assert_eq!(
        records[0].permission_matcher_input.as_deref(),
        Some("cargo test -q")
    );
    assert_eq!(
        records[0].transcript_summary.as_deref(),
        Some("cargo test -q")
    );
    assert_eq!(records[0].ui_render_kind.as_deref(), Some("shell"));
    assert_eq!(
        records[0].relevance,
        ToolExecutionRelevance {
            validation: true,
            closeout: true,
            repair: false,
            policy: ToolExecutionRelevancePolicyRecord {
                route_workflow: Some("code_change".to_string()),
                closeout_reasons: vec!["validation".to_string()],
                repair_reasons: Vec::new(),
            },
        }
    );
    assert_eq!(
        records[0]
            .terminal_task
            .as_ref()
            .and_then(|task| task.task_id.as_deref()),
        Some("shell_foreground_123")
    );
    assert_eq!(
        records[0]
            .execution
            .route
            .as_ref()
            .and_then(|route| route.workflow.as_deref()),
        Some("code_change")
    );
    assert_eq!(
        records[0]
            .execution
            .policy
            .as_ref()
            .and_then(|policy| policy.max_tool_calls),
        Some(30)
    );
    assert!(records[0].execution.action_checkpoint_active);
    assert!(records[0].execution.has_changes_before_tools);
    assert_eq!(records[0].execution.exposed_tools_count, Some(15));
    assert_eq!(
        records[0].execution.started_at_unix_ms,
        Some(1_770_000_000_000)
    );
    assert_eq!(
        records[0].execution.finished_at_unix_ms,
        Some(1_770_000_000_042)
    );
}

#[test]
fn records_denied_permission_execution_record() {
    let mut ledger = EvidenceLedger::new();
    let mut result = ToolResult::error("permission denied");
    result.data = Some(serde_json::json!({
        "permission_request": {
            "id": "git_push",
            "session_id": "session-1",
            "kind": "write",
            "approved": false,
            "patterns": ["file_write"],
            "allowed_always_rules": ["file_read"],
            "metadata": {
                "permission_requires": true,
                "tool_requires": false,
                "raw_tool_requires": false,
                "drift_requires_approval": false,
                "permission_family": "file",
                "permission_decision": "Ask",
                "permission_source": "config_project_ask",
                "resolved_permission_source": "user_once_reject",
                "risk_level": "High"
            },
            "rejection_feedback": "Denied by policy"
        }
    }));

    ledger.record_tool_result(
        &tool_call("file_write", serde_json::json!({"path": "src/lib.rs"})),
        &result,
    );

    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.status, ToolExecutionStatus::Denied);
    assert_eq!(
        record.relevance,
        ToolExecutionRelevance {
            validation: false,
            closeout: true,
            repair: true,
            policy: ToolExecutionRelevancePolicyRecord {
                route_workflow: None,
                closeout_reasons: vec!["permission".to_string()],
                repair_reasons: vec!["tool_failed".to_string()],
            },
        }
    );
    assert_eq!(
        record
            .permission
            .as_ref()
            .and_then(|permission| permission.kind.as_deref()),
        Some("write")
    );
    let permission = record.permission.as_ref().unwrap();
    assert!(!permission.approved);
    assert_eq!(permission.request_id.as_deref(), Some("git_push"));
    assert_eq!(permission.session_id.as_deref(), Some("session-1"));
    assert_eq!(permission.patterns, vec!["file_write"]);
    assert_eq!(permission.allowed_always_rules, vec!["file_read"]);
    assert_eq!(permission.source.permission_requires, Some(true));
    assert_eq!(permission.source.permission_family.as_deref(), Some("file"));
    assert_eq!(
        permission.source.permission_decision.as_deref(),
        Some("Ask")
    );
    assert_eq!(
        permission.source.permission_source.as_deref(),
        Some("config_project_ask")
    );
    assert_eq!(
        permission.source.resolved_permission_source.as_deref(),
        Some("user_once_reject")
    );
    assert_eq!(permission.source.risk_level.as_deref(), Some("High"));
    assert_eq!(ledger.snapshot().denied_permission_facts, 1);
}

#[test]
fn approved_permission_record_keeps_failed_tool_status_failed() {
    let mut ledger = EvidenceLedger::new();
    let mut result = ToolResult::error("remote rejected push");
    result.data = Some(serde_json::json!({
        "permission_request": {
            "id": "git_push",
            "session_id": "session-1",
            "kind": "runtime_rule",
            "approved": true,
            "patterns": ["git"],
            "metadata": {
                "permission_requires": true,
                "tool_requires": false,
                "raw_tool_requires": false,
                "drift_requires_approval": false,
                "permission_family": "other",
                "permission_decision": "Ask",
                "risk_level": "High"
            },
            "rejection_feedback": "Permission denied: 'git' requires user confirmation."
        }
    }));

    ledger.record_tool_result(
        &tool_call("git", serde_json::json!({"action": "push"})),
        &result,
    );

    let record = &ledger.tool_execution_records()[0];
    assert_eq!(record.status, ToolExecutionStatus::Failed);
    assert!(record.permission.as_ref().unwrap().approved);
    assert_eq!(ledger.snapshot().permission_facts, 1);
    assert_eq!(ledger.snapshot().denied_permission_facts, 0);
}

#[test]
fn records_shell_assertion_as_runtime_validation() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({
                "command": "test -d fixtures/core_quality/inspection_target/gex && echo PASS"
            }),
        ),
        &ToolResult::success("PASS: directory exists"),
    );

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.command_facts, 1);
    assert_eq!(snapshot.validation_facts, 1);
    assert_eq!(snapshot.passed_validation_facts, 1);
    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("passed:1/1")
    );
}

#[test]
fn required_validation_label_uses_required_commands_over_exploratory_failures() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({
                "command": "python3 -c \"import core_terminal_demo; print('import ok')\""
            }),
        ),
        &ToolResult::error("ModuleNotFoundError: No module named 'core_terminal_demo'"),
    );
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({
                "command": "test -x .venv/bin/python && echo \"PASS: .venv/bin/python exists\""
            }),
        ),
        &ToolResult::success("PASS: .venv/bin/python exists"),
    );
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({
                "command": ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'"
            }),
        ),
        &ToolResult::success("core-terminal-demo-ok"),
    );
    let required = vec![
        "test -x .venv/bin/python".to_string(),
        ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'".to_string(),
    ];

    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("failed:1/2")
    );
    assert_eq!(
        ledger
            .runtime_required_validation_label(&required)
            .as_deref(),
        Some("passed:2/2")
    );
}

#[test]
fn verification_proof_reports_missing_required_commands_as_not_run() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
        &ToolResult::success("test result: ok"),
    );
    let required = vec!["cargo test -q".to_string(), "cargo fmt --check".to_string()];

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &required,
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: VerificationProofSupportContext::code_change(),
    });

    assert_eq!(proof.status, VerificationProofStatus::NotRun);
    assert_eq!(proof.required_total, 2);
    assert_eq!(proof.required_passed, 1);
    assert_eq!(proof.required_missing, 1);
    assert_eq!(
        proof.missing_required_commands,
        vec!["cargo fmt --check".to_string()]
    );
    assert!(proof
        .validation_line()
        .contains("verification proof: not_run"));
}

#[test]
fn verification_proof_prefers_required_validation_success_over_prior_user_deferred_state() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_validation_result(
        "run_tests",
        Some("python3 fixtures/example/test_slugify.py"),
        true,
        "OK",
    );
    let required = vec!["python3 fixtures/example/test_slugify.py".to_string()];

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &required,
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::UserDeferred,
        support_context: VerificationProofSupportContext::code_change(),
    });

    assert_eq!(proof.status, VerificationProofStatus::Verified);
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Verified
    );
    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::CommandPassed));
    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::RequiredValidationPassed));
    assert_eq!(proof.required_passed, 1);
    assert!(proof.summary.contains("required validation passed 1/1"));
}

#[test]
fn verification_proof_does_not_trust_verified_task_state_without_ledger_evidence() {
    let ledger = EvidenceLedger::new();

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Verified,
        support_context: VerificationProofSupportContext::code_change(),
    });

    assert_eq!(proof.status, VerificationProofStatus::Unavailable);
    assert!(proof
        .summary
        .contains("ledger has no verification evidence"));
}

#[test]
fn records_bash_validation_from_result_metadata_when_call_arguments_are_missing() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "PASS: directory exists",
        serde_json::json!({
            "shell_result": {
                "command": "if test -d fixtures/core_quality/inspection_target/gex; then echo PASS; else echo FAIL; fi"
            }
        }),
    );

    ledger.record_tool_result(
        &ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({}),
        },
        &result,
    );

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.command_facts, 1);
    assert_eq!(snapshot.validation_facts, 1);
    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("passed:1/1")
    );
}

#[test]
fn records_agent_tool_result_as_subagent_claim_only_proof() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "Sub-agent agent_1 completed with status: Completed",
        serde_json::json!({
            "agent_id": "agent_1",
            "source_agent": "agent_1",
            "status": "completed",
            "result": "review says tests look good",
            "proof_kind": "subagent_claim_only",
            "verification_proof_kind": "subagent_claim_only",
            "subagent_output_kind": "SubagentVerificationClaim",
            "parent_verified": false,
            "scope": "subagent_result",
            "related_to_changed_files": "none",
            "residual_risk": "subagent output is a claim until parent runtime verification"
        }),
    );

    ledger.record_tool_result(
        &tool_call(
            "agent",
            serde_json::json!({"description": "review", "prompt": "check"}),
        ),
        &result,
    );

    let facts = ledger.validation_facts();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].source, "agent:agent_1");
    assert!(facts[0].passed);
    assert_eq!(
        facts[0].proof_kind,
        Some(VerificationProofKind::SubagentClaimOnly)
    );
    assert_eq!(facts[0].source_agent.as_deref(), Some("agent_1"));
    assert_eq!(facts[0].parent_verified, Some(false));
    assert!(ledger.tool_execution_records()[0].relevance.validation);
    assert!(ledger.tool_execution_records()[0].relevance.closeout);

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: VerificationProofSupportContext::code_change(),
    });

    assert_eq!(proof.status, VerificationProofStatus::Verified);
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Partial
    );
    assert!(!proof.derived_support.supports_verified);
    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::SubagentClaimOnly));
}

#[test]
fn parent_runtime_validation_does_not_promote_subagent_claim_without_explicit_parent_record() {
    let mut ledger = EvidenceLedger::new();
    let subagent_result = ToolResult::success_with_data(
        "Sub-agent agent_1 completed with status: Completed",
        serde_json::json!({
            "agent_id": "agent_1",
            "source_agent": "agent_1",
            "status": "completed",
            "result": "review says the target behavior is present",
            "verification_proof_kind": "subagent_claim_only",
            "subagent_output_kind": "SubagentVerificationClaim",
            "parent_verified": false,
            "scope": "subagent_result",
        }),
    );
    ledger.record_tool_result(
        &tool_call(
            "agent",
            serde_json::json!({"description": "review", "prompt": "check"}),
        ),
        &subagent_result,
    );

    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "cargo check -q"})),
        &ToolResult::success("cargo check finished successfully"),
    );

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: ledger
            .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
    });

    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::SubagentClaimOnly));
    assert!(!proof
        .proof_kinds
        .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Partial
    );
    assert!(!proof.derived_support.supports_verified);
}

#[test]
fn non_validation_parent_command_does_not_promote_subagent_claim() {
    let mut ledger = EvidenceLedger::new();
    let subagent_result = ToolResult::success_with_data(
        "Sub-agent agent_1 completed with status: Completed",
        serde_json::json!({
            "agent_id": "agent_1",
            "status": "completed",
            "result": "review says the target behavior is present",
            "verification_proof_kind": "subagent_claim_only",
            "subagent_output_kind": "SubagentVerificationClaim",
            "parent_verified": false,
        }),
    );
    ledger.record_tool_result(
        &tool_call(
            "agent",
            serde_json::json!({"description": "review", "prompt": "check"}),
        ),
        &subagent_result,
    );
    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "echo inspected"})),
        &ToolResult::success("inspected"),
    );

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: ledger
            .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
    });

    assert!(!proof
        .proof_kinds
        .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Partial
    );
    assert!(!proof.derived_support.supports_verified);
}

#[test]
fn records_parent_verified_subagent_result_as_verified_support() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "Parent runtime verified sub-agent agent_1",
        serde_json::json!({
            "agent_id": "agent_1",
            "source_agent": "agent_1",
            "status": "verified",
            "result": "parent reran focused checks",
            "verification_proof_kind": "parent_verified_subagent_result",
            "subagent_output_kind": "SubagentVerificationClaim",
            "parent_verified": true,
            "scope": "parent_runtime_verification",
            "claim_id": "claim_agent_1_compile",
            "claim_type": "compile_check",
            "parent_command": "cargo check -q",
            "artifact_ids": ["tool_run_789"],
            "verification_verdict": "verified_for_compile_only",
            "verified_at": "2026-05-26T00:00:00Z",
            "related_to_changed_files": "yes",
            "residual_risk": "parent runtime verified subagent result"
        }),
    );

    ledger.record_tool_result(
        &tool_call("agent", serde_json::json!({"action": "resume"})),
        &result,
    );

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: ledger
            .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
    });

    assert_eq!(proof.status, VerificationProofStatus::Verified);
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Verified
    );
    assert!(proof.derived_support.supports_verified);
    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
    let fact = &ledger.validation_facts()[0];
    assert_eq!(fact.claim_id.as_deref(), Some("claim_agent_1_compile"));
    assert_eq!(fact.artifact_ids, vec!["tool_run_789".to_string()]);
    assert_eq!(
        fact.verification_verdict.as_deref(),
        Some("verified_for_compile_only")
    );
}

#[test]
fn unbound_parent_verified_subagent_record_is_downgraded_to_claim_only() {
    let mut ledger = EvidenceLedger::new();
    let result = ToolResult::success_with_data(
        "Parent runtime verified sub-agent agent_1",
        serde_json::json!({
            "agent_id": "agent_1",
            "source_agent": "agent_1",
            "status": "verified",
            "result": "parent says checks passed but does not bind the claim",
            "verification_proof_kind": "parent_verified_subagent_result",
            "subagent_output_kind": "SubagentVerificationClaim",
            "parent_verified": true,
            "scope": "parent_runtime_verification",
            "related_to_changed_files": "yes"
        }),
    );

    ledger.record_tool_result(
        &tool_call("agent", serde_json::json!({"action": "resume"})),
        &result,
    );

    let proof = ledger.verification_proof(VerificationProofRequest {
        required_commands: &[],
        requires_validation: true,
        task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
        support_context: ledger
            .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
    });

    assert!(proof
        .proof_kinds
        .contains(&VerificationProofKind::SubagentClaimOnly));
    assert!(!proof
        .proof_kinds
        .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
    assert_eq!(
        proof.derived_support.status,
        VerificationProofStatus::Partial
    );
    assert!(!proof.derived_support.supports_verified);
    let fact = &ledger.validation_facts()[0];
    assert_eq!(
        fact.proof_kind,
        Some(VerificationProofKind::SubagentClaimOnly)
    );
    assert_eq!(fact.parent_verified, Some(false));
}

#[test]
fn records_permission_denial_as_permission_fact() {
    let mut ledger = EvidenceLedger::new();
    let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
    result.error_code = Some(crate::tools::ToolErrorCode::PermissionDenied);
    result.data = Some(serde_json::json!({
        "permission_request": {
            "kind": "runtime_rule",
            "permission_source": "hook_deny",
            "rejection_feedback": "Permission denied: 'git' requires user confirmation.",
            "recovery_feedback": "Ask the user to approve git push before retrying."
        }
    }));
    ledger.record_tool_result(
        &tool_call("git", serde_json::json!({"action": "push"})),
        &result,
    );

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.permission_facts, 1);
    assert_eq!(snapshot.denied_permission_facts, 1);
    assert_eq!(ledger.permission_facts()[0].tool, "git");
    assert_eq!(
        ledger.permission_facts()[0].kind.as_deref(),
        Some("runtime_rule")
    );
    assert!(ledger.permission_facts()[0]
        .summary
        .contains("Recovery: Ask the user"));
    assert_eq!(
        ledger.permission_facts()[0].source.as_deref(),
        Some("hook_deny")
    );
}

#[test]
fn failed_validation_label_names_failures() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_validation_result("auto_verify", Some("cargo check"), false, "compile error");

    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("failed:1/1")
    );
    assert_eq!(ledger.validation_facts()[0].summary, "compile error");
}

#[test]
fn repair_tool_record_evidence_uses_failed_and_changed_records() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call(
            "grep",
            serde_json::json!({"pattern": "ok", "path": "src/lib.rs"}),
        ),
        &ToolResult::success("src/lib.rs:1:ok"),
    );
    ledger.record_tool_result(
        &tool_call("file_edit", serde_json::json!({"path": "src/lib.rs"})),
        &ToolResult::success("File edited successfully: src/lib.rs"),
    );
    ledger.record_tool_result(
        &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
        &ToolResult::error_with_content("command exited 101", "test failed"),
    );

    let evidence = ledger.repair_tool_record_evidence(&["cargo test -q".to_string()]);

    assert_eq!(evidence.len(), 2);
    assert!(evidence[0].contains("tool=bash"));
    assert!(evidence[0].contains("status=failed"));
    assert!(evidence[0].contains("command=cargo test -q"));
    assert!(evidence[1].contains("tool=file_edit"));
    assert!(!evidence.iter().any(|item| item.contains("tool=grep")));
}

#[test]
fn runtime_validation_label_uses_latest_result_per_command() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_validation_result(
        "bash",
        Some("cargo test -q tui -- --test-threads=1"),
        false,
        "provider header panic",
    );
    ledger.record_validation_result(
        "required_validation",
        Some("cargo    test -q tui -- --test-threads=1"),
        true,
        "test result: ok",
    );
    ledger.record_validation_result("code_review", None, true, "review passed");

    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("passed:2/2 recovered_failed:1")
    );
    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.validation_facts, 3);
    assert_eq!(snapshot.failed_validation_facts, 1);
}

#[test]
fn runtime_validation_label_keeps_unrecovered_failures_current() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_validation_result(
        "bash",
        Some("cargo test -q tui -- --test-threads=1"),
        false,
        "test failed",
    );
    ledger.record_validation_result(
        "required_validation",
        Some("cargo test -q shell -- --test-threads=1"),
        true,
        "test result: ok",
    );

    assert_eq!(
        ledger.runtime_validation_label().as_deref(),
        Some("failed:1/2")
    );
}

#[tokio::test]
async fn changed_files_diff_evidence_skips_empty_input() {
    let evidence = changed_files_diff_evidence(Path::new("."), &[]).await;

    assert!(evidence.is_none());
}

#[test]
fn filesystem_grounding_flags_creation_time_without_evidence() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({"command": "ls -la ~/Desktop | grep -i gex"}),
        ),
        &ToolResult::success("drwxr-xr-x  3 gex  staff  96 May 8  2024 gex"),
    );

    let gaps = ledger.unsupported_filesystem_claims("创建时间：2024 年 5 月 8 日");

    assert_eq!(gaps, vec!["creation_time".to_string()]);
}

#[test]
fn filesystem_grounding_allows_creation_time_with_stat_evidence() {
    let mut ledger = EvidenceLedger::new();
    ledger.record_tool_result(
        &tool_call(
            "bash",
            serde_json::json!({"command": "stat -f '%SB' ~/Desktop/gex"}),
        ),
        &ToolResult::success("May 8 00:00:00 2024\ncreated at"),
    );

    let gaps = ledger.unsupported_filesystem_claims("创建时间：May 8 00:00:00 2024");

    assert!(gaps.is_empty());
}

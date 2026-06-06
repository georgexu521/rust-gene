use super::*;

fn tool_call(name: &str) -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: name.to_string(),
        arguments: serde_json::json!({"command": "cargo test -q"}),
    }
}

#[tokio::test]
async fn appends_provider_tool_result_and_records_evidence() {
    let mut ledger = EvidenceLedger::new();
    let mut tool_results_text = String::new();
    let mut messages = Vec::new();
    let mut result = ToolResult::success("ok");

    append_provider_tool_result(
        &tool_call("bash"),
        &mut result,
        &mut ledger,
        &mut tool_results_text,
        &mut messages,
    )
    .await;

    assert_eq!(tool_results_text, "Result: OK\nok\n");
    assert_eq!(ledger.snapshot().command_facts, 1);
    assert_eq!(ledger.snapshot().validation_facts, 1);
    assert_eq!(messages.len(), 1);
    assert!(matches!(
        &messages[0],
        Message::Tool {
            tool_call_id,
            content
        } if tool_call_id == "call_1" && content == "Result: OK\nok"
    ));
}

#[tokio::test]
async fn normalize_after_execution_truncates_large_output_with_metadata() {
    let mut result = ToolResult::success("A".repeat(40_000));
    let normalized = ToolResultNormalizer::normalize_after_execution(
        &ToolCall {
            id: "call_large".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({"pattern": "A", "path": "src"}),
        },
        &mut result,
    )
    .await;

    assert!(normalized.model_content.contains("Output truncated"));
    assert_eq!(
        normalized.structured_metadata["tool_result_data"]["output_truncation"]["original_bytes"],
        40_000
    );
    assert!(
        normalized.structured_metadata["tool_result_data"]["output_truncation"]["output_uri"]
            .as_str()
            .unwrap_or_default()
            .contains("tool-output://")
    );
    assert!(normalized.context_policy.compaction_eligible);
    assert!(normalized
        .context_policy
        .durable_artifact_path
        .as_deref()
        .unwrap_or_default()
        .contains("tool-output://"));
    assert_eq!(
        normalized.structured_metadata["tool_result_context_policy"]["compaction_eligible"],
        true
    );
}

#[test]
fn normalizes_provider_tool_result_content() {
    let normalized =
        ToolResultNormalizer::normalize(&tool_call("bash"), &ToolResult::success("ok"));

    assert_eq!(normalized.model_content, "Result: OK\nok");
    assert_eq!(normalized.ui_content, "Result: OK\nok");
    assert_eq!(
        normalized.evidence_facts,
        vec![
            NormalizedEvidenceFact::Command,
            NormalizedEvidenceFact::Validation
        ]
    );
    assert_eq!(normalized.structured_metadata["tool"], "bash");
    assert_eq!(normalized.structured_metadata["call_id"], "call_1");
    assert_eq!(normalized.structured_metadata["success"], true);
    assert_eq!(normalized.structured_metadata["error_code"], "success");
    assert!(normalized.structured_metadata.get("tool_summary").is_some());
    assert_eq!(normalized.observation.status, "success");
    assert_eq!(
        normalized.observation.command_run.as_deref(),
        Some("cargo test -q")
    );
    assert_eq!(
        normalized.observation.validation_result.as_deref(),
        Some("passed")
    );
    assert_eq!(
        normalized.structured_metadata["tool_observation"]["status"],
        "success"
    );
    assert!(normalized.context_policy.ledger_fact_eligible);
    assert!(!normalized.context_policy.protected_recent_tail);
}

#[test]
fn attach_observation_metadata_writes_compact_result_state() {
    let mut result = ToolResult::success_with_data(
        "Edited src/app.rs",
        serde_json::json!({
            "checkpoint": {"id": "cp_1"},
            "diff": {"additions": 1, "deletions": 0}
        }),
    );
    let tool_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: serde_json::json!({"path": "src/app.rs"}),
    };

    ToolResultNormalizer::attach_observation_metadata(&tool_call, &mut result);

    let observation = &result.data.as_ref().unwrap()["tool_observation"];
    assert_eq!(observation["status"], "success");
    assert_eq!(observation["files_changed"][0], "src/app.rs");
    assert_eq!(observation["checkpoint_id"], "cp_1");
    assert_eq!(observation["state_updates"][0], "files_changed");
    assert_eq!(
        result.data.as_ref().unwrap()["tool_result_context_policy"]["model_visibility"],
        "full_raw"
    );
}

#[test]
fn context_policy_protects_failed_and_mutating_tool_results() {
    let failed = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_fail".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "cargo test -q"}),
        },
        &ToolResult::error("tests failed"),
    );
    assert!(failed.context_policy.protected_recent_tail);

    let edit = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        },
        &ToolResult::success("edited src/lib.rs"),
    );
    assert!(edit.context_policy.protected_recent_tail);
    assert!(edit.context_policy.ledger_fact_eligible);
}

#[test]
fn normalizes_bash_validation_from_result_metadata_when_arguments_are_missing() {
    let result = ToolResult::success_with_data(
        "PASS: directory exists",
        serde_json::json!({
            "shell_result": {
                "command": "if test -d fixtures/core_quality/inspection_target/gex; then echo PASS; else echo FAIL; fi"
            }
        }),
    );
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_from_result".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({}),
        },
        &result,
    );

    assert_eq!(
        normalized.evidence_facts,
        vec![
            NormalizedEvidenceFact::Command,
            NormalizedEvidenceFact::Validation
        ]
    );
}

#[test]
fn observes_failed_validation_with_findings_evidence_and_repair_attention() {
    let result = ToolResult::error_with_content(
        "cargo test failed",
        "running 2 tests\n\
         test auth::login --- FAILED\n\
         ---- auth::session stdout ----\n\
         thread 'auth::session' panicked at src/auth/session.rs:42: token missing\n\
         error[E0425]: cannot find value `token` in this scope",
    );
    let normalized = ToolResultNormalizer::normalize(&tool_call("bash"), &result);

    assert_eq!(normalized.observation.result_kind, "validation");
    assert_eq!(
        normalized.observation.validation_result.as_deref(),
        Some("failed")
    );
    assert!(normalized
        .observation
        .key_findings
        .iter()
        .any(|finding| finding.contains("Failed tests: auth::login")));
    assert!(normalized
        .observation
        .evidence
        .iter()
        .any(|evidence| evidence.text.contains("error[E0425]")));
    assert!(normalized
        .observation
        .next_attention
        .iter()
        .any(|item| item.contains("Rerun `cargo test -q`")));
    assert!(!normalized.observation.hypothesis_updates.is_empty());
    assert_eq!(normalized.context_policy.model_visibility, "raw_excerpt");
    assert!(normalized
        .model_content
        .contains("Observation (validation)"));
    assert!(normalized.model_content.contains("Raw excerpt:"));
}

#[test]
fn observes_noisy_search_as_observation_first_with_top_matches() {
    let matches = (1..=12)
        .map(|line| {
            serde_json::json!({
                "file": if line <= 6 { "src/auth/login.rs" } else { "src/auth/session.rs" },
                "line": line,
                "content": format!("fn match_{line}() {{}}"),
            })
        })
        .collect::<Vec<_>>();
    let result = ToolResult::success_with_data(
        (1..=12)
            .map(|line| format!("{line:4}: fn match_{line}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::json!({
            "pattern": "match_",
            "path": "src",
            "kind": "search",
            "total_matches": 12,
            "truncated": false,
            "matches": matches,
        }),
    );
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_search".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({"pattern": "match_", "path": "src"}),
        },
        &result,
    );

    assert_eq!(normalized.observation.result_kind, "search");
    assert!(normalized
        .observation
        .key_findings
        .iter()
        .any(|finding| finding.contains("Search found 12 match")));
    assert!(normalized
        .observation
        .candidate_focus
        .contains(&"src/auth/login.rs".to_string()));
    assert_eq!(normalized.context_policy.model_visibility, "observation");
    assert!(normalized.model_content.contains("Observation (search)"));
    assert!(!normalized.model_content.contains("match_12"));
}

#[test]
fn observes_successful_edit_with_diff_summary_and_validation_attention() {
    let result = ToolResult::success_with_data(
        "File edited successfully: src/app.rs (1 replacement(s))",
        serde_json::json!({
            "path": "src/app.rs",
            "checkpoint": {"id": "cp_1"},
            "replacements": 1,
            "diff": {
                "additions": 2,
                "deletions": 1,
                "changed_line_start": 10,
                "changed_line_end": 12,
                "unified_diff": "--- a/src/app.rs\n+++ b/src/app.rs\n-old\n+new"
            }
        }),
    );
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "src/app.rs"}),
        },
        &result,
    );

    assert_eq!(normalized.observation.result_kind, "edit");
    assert_eq!(normalized.observation.files_changed, vec!["src/app.rs"]);
    assert!(normalized
        .observation
        .key_findings
        .iter()
        .any(|finding| finding.contains("Diff summary: +2 -1")));
    assert!(normalized
        .observation
        .next_attention
        .iter()
        .any(|item| item.contains("Verify the change")));
    assert_eq!(
        normalized.observation.checkpoint_id.as_deref(),
        Some("cp_1")
    );
}

#[test]
fn observes_unknown_bash_without_validation_evidence() {
    let result = ToolResult::error_with_content(
        "custom command failed",
        "custom tool wrote partial output before failing",
    );
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_unknown".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "custom-tool --maybe-mutates"}),
        },
        &result,
    );

    assert_eq!(normalized.observation.result_kind, "unknown_command");
    assert_eq!(normalized.observation.validation_result, None);
    assert!(normalized.observation.risk_note.is_some());
    assert_eq!(
        normalized.evidence_facts,
        vec![NormalizedEvidenceFact::Command]
    );
    assert_eq!(normalized.context_policy.model_visibility, "raw_excerpt");
    assert!(normalized
        .model_content
        .contains("not classified as validation"));
}

#[test]
fn invalid_params_result_carries_schema_validation_metadata() {
    let result =
        invalid_tool_params_result(&tool_call("bash"), "Missing required parameter: command");
    let normalized = ToolResultNormalizer::normalize(&tool_call("bash"), &result);

    assert!(!result.success);
    assert_eq!(
        normalized.structured_metadata["error_code"],
        "invalid_params"
    );
    assert_eq!(
        normalized.structured_metadata["tool_result_data"]["schema_validation"]["valid"],
        false
    );
}

#[test]
fn normalizes_file_write_evidence_categories() {
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_2".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path": "src/app.rs"}),
        },
        &ToolResult::success("Wrote file"),
    );

    assert_eq!(
        normalized.evidence_facts,
        vec![
            NormalizedEvidenceFact::File,
            NormalizedEvidenceFact::ChangedFile
        ]
    );
}

#[test]
fn normalizes_permission_denied_evidence_category() {
    let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
    result.data = Some(serde_json::json!({
        "permission_request": {
            "kind": "runtime_rule",
            "permission_source": "hook_deny",
            "rejection_feedback": "Permission denied: 'git' requires user confirmation."
        }
    }));
    let normalized = ToolResultNormalizer::normalize(
        &ToolCall {
            id: "call_permission".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({"action": "push"}),
        },
        &result,
    );

    assert_eq!(
        normalized.evidence_facts,
        vec![NormalizedEvidenceFact::Permission]
    );
    assert_eq!(
        normalized.structured_metadata["tool_result_data"]["permission_request"]["kind"],
        "runtime_rule"
    );
    assert_eq!(
        normalized.observation.permission_source.as_deref(),
        Some("hook_deny")
    );
    assert!(normalized
        .observation
        .next_attention
        .iter()
        .any(|item| item.contains("Ask for approval")));
    assert!(normalized.observation.quality_warnings.is_empty());
}

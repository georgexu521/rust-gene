use super::*;
use crate::services::api::{ChatRequest, ChatResponse, ToolCall, Usage};
use crate::test_utils::env_guard::EnvVarGuard;
use crate::tools::{BashTool, FileEditTool, FileReadTool, FileWriteTool, GitTool};
use async_openai::types::ChatCompletionResponseStream;
use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex as StdMutex, OnceLock};
use tempfile::tempdir;

struct CapabilityProbeProvider {
    base_url: &'static str,
    model: &'static str,
}

#[async_trait::async_trait]
impl LlmProvider for CapabilityProbeProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        Err(anyhow::anyhow!("chat not used in capability probe"))
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        Err(anyhow::anyhow!("chat_stream not used in capability probe"))
    }

    fn base_url(&self) -> &str {
        self.base_url
    }

    fn default_model(&self) -> &str {
        self.model
    }
}

#[test]
fn memory_use_and_generate_controls_are_independent() {
    let memory_manager = Arc::new(Mutex::new(crate::memory::MemoryManager::with_base_dir(
        tempdir().unwrap().path().to_path_buf(),
    )));
    let conversation = ConversationLoop::new(
        Arc::new(CapabilityProbeProvider {
            base_url: "https://api.openai.com/v1",
            model: "gpt-test",
        }),
        Arc::new(ToolRegistry::new()),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "gpt-test".to_string(),
    )
    .with_memory_manager(memory_manager)
    .with_memory_use(false)
    .with_memory_generate(true);

    assert!(conversation.memory_manager_for_static_memory().is_none());
    assert!(conversation.memory_manager_for_dynamic_recall().is_none());
    assert!(conversation.memory_manager_for_generate().is_some());
}

#[test]
fn recall_off_keeps_static_memory_snapshot_available() {
    let memory_manager = Arc::new(Mutex::new(crate::memory::MemoryManager::with_base_dir(
        tempdir().unwrap().path().to_path_buf(),
    )));
    let conversation = ConversationLoop::new(
        Arc::new(CapabilityProbeProvider {
            base_url: "https://api.openai.com/v1",
            model: "gpt-test",
        }),
        Arc::new(ToolRegistry::new()),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "gpt-test".to_string(),
    )
    .with_memory_manager(memory_manager)
    .with_memory_use(true)
    .with_memory_recall_mode("off");

    assert!(conversation.memory_manager_for_static_memory().is_some());
    assert!(conversation.memory_manager_for_dynamic_recall().is_none());
}

#[tokio::test]
async fn test_truncate_tool_result_handles_utf8_boundaries() {
    let mut result = ToolResult::success("中".repeat(20_000));
    truncate_tool_result(
        &mut result,
        "grep",
        "call_utf8",
        None,
        std::path::Path::new("."),
    )
    .await;
    assert!(result.content.contains("Output truncated"));
}

#[test]
fn nonstreaming_tool_routing_uses_provider_capabilities() {
    let tools = vec![crate::services::api::Tool::new("bash", "run shell command")];
    let minimax = CapabilityProbeProvider {
        base_url: "https://api.minimaxi.com/v1",
        model: "MiniMax-M2.7",
    };
    let openai = CapabilityProbeProvider {
        base_url: "https://api.openai.com/v1",
        model: "gpt-4o",
    };

    assert!(should_use_nonstreaming_tools(&minimax, &tools));
    assert!(!should_use_nonstreaming_tools(&openai, &tools));
    assert!(!should_use_nonstreaming_tools(&minimax, &[]));
}

#[tokio::test]
async fn test_required_validation_shell_strips_agent_runtime_env() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_AUTO_TEST", "check_then_test");
    env.set(
        "PRIORITY_AGENT_EVAL_EVENTS",
        "/tmp/priority-agent-events.jsonl",
    );

    let tmp = tempdir().expect("create temp dir");
    let output = shell_output_with_timeout(
            "printf '%s:%s' \"${PRIORITY_AGENT_AUTO_TEST:-unset}\" \"${PRIORITY_AGENT_EVAL_EVENTS:-unset}\"",
            tmp.path(),
            Some(std::time::Duration::from_secs(5)),
        )
        .await
        .expect("run shell command");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "unset:unset");
}

#[test]
fn test_extract_required_validation_commands_keeps_live_eval_script_checks() {
    let prompt = r#"
## Acceptance checks
- `bash -n scripts/run_live_eval.sh`
- `scripts/run_live_eval.sh --list`
- `scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke`
- `cargo test -q -- --test-threads=1`
"#;

    let commands = RequiredValidationController::extract_commands(prompt);
    assert_eq!(
        commands,
        vec![
            "bash -n scripts/run_live_eval.sh",
            "scripts/run_live_eval.sh --list",
            "scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke",
            "cargo test -q -- --test-threads=1",
        ]
    );
}

#[test]
fn test_extract_required_validation_commands_from_interactive_chinese_prompt() {
    let prompt = "请修复 todo.py，让测试期望的小写 todo 前缀通过。只改 todo.py，运行 python3 -m unittest test_todo.py 验证。";

    let commands = RequiredValidationController::extract_commands(prompt);

    assert_eq!(commands, vec!["python3 -m unittest test_todo.py"]);
}

#[test]
fn test_extract_required_validation_commands_from_inline_backticks() {
    let prompt = "改完后请运行 `cargo test -q shell -- --test-threads=1`，不要改其他文件。";

    let commands = RequiredValidationController::extract_commands(prompt);

    assert_eq!(commands, vec!["cargo test -q shell -- --test-threads=1"]);
}

#[test]
fn test_not_allowed_tool_result_has_recovery_metadata() {
    let tool_call = ToolCall {
        id: "call_denied".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({"command": "echo hi"}),
    };
    let result = tool_not_allowed_result(&tool_call);
    assert!(!result.success);
    assert!(result
        .error
        .as_deref()
        .unwrap_or("")
        .contains("not allowed"));
    let data = result.data.expect("tool summary data");
    assert_eq!(data["tool_summary"]["tool"], "bash");
    assert_eq!(data["tool_summary"]["call_id"], "call_denied");
}

#[test]
fn test_tool_recovery_metadata_attached_to_failure() {
    let mut result = ToolResult::error("command timed out");
    let tool_call = ToolCall {
        id: "call_bash".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "cargo test -q"
        }),
    };
    attach_tool_execution_metadata(&tool_call, &mut result);
    assert_eq!(result.content, "command timed out");
    let summary = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .expect("tool summary metadata");
    assert_eq!(summary["tool"], "bash");
    assert_eq!(summary["command_kind"], "validation");
    assert_eq!(summary["command_category"], "test_run");
    assert_eq!(summary["validation_family"], "cargo_test");
    assert_eq!(summary["safe_for_closeout"], true);
    assert_eq!(summary["network_access"], false);
    assert_eq!(summary["external_path_access"], false);
    assert_eq!(summary["expected_silent_output"], false);
    assert_eq!(
        summary["permission_rule_suggestions"][1]["pattern"],
        "cargo test"
    );
    let recovery = result
        .data
        .as_ref()
        .and_then(|data| data.get("recovery"))
        .expect("recovery metadata");
    assert_eq!(recovery["recoverable"], true);
    assert_eq!(recovery["safe_retry"], true);
    assert_eq!(recovery["suggested_command"], "/retry");
}

#[test]
fn test_tool_summary_metadata_attached_to_success() {
    let mut result = ToolResult::success_with_data(
        "File edited successfully",
        serde_json::json!({
            "path": "src/lib.rs",
            "replacements": 1
        }),
    );
    let tool_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    };
    attach_tool_execution_metadata(&tool_call, &mut result);
    let summary = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .expect("tool summary metadata");
    assert_eq!(summary["tool"], "file_edit");
    assert_eq!(summary["path"], "src/lib.rs");
    assert_eq!(summary["replacements"], 1);
    assert!(result
        .data
        .as_ref()
        .and_then(|data| data.get("recovery"))
        .is_none());
}

#[test]
fn test_tool_execution_start_progress_uses_validation_labels() {
    assert_eq!(
        tool_execution_start_progress(
            "bash",
            &serde_json::json!({"command": "cargo test -q -- --test-threads=1"})
        ),
        "Running Rust tests: cargo test -q -- --test-threads=1"
    );
    assert_eq!(
        tool_execution_start_progress(
            "bash",
            &serde_json::json!({"command": "env PRIORITY_AGENT=1 cargo check -q"})
        ),
        "Running cargo check: env PRIORITY_AGENT=1 cargo check -q"
    );
    assert_eq!(
        tool_execution_start_progress(
            "bash",
            &serde_json::json!({"command": "cargo clippy -q -- -D warnings"})
        ),
        "Running cargo clippy: cargo clippy -q -- -D warnings"
    );
}

#[test]
fn test_tool_execution_start_progress_handles_generic_shell_and_tools() {
    assert_eq!(
        tool_execution_start_progress("bash", &serde_json::json!({"command": "ls src"})),
        "Listing with shell: ls src"
    );
    assert_eq!(
        tool_execution_start_progress(
            "bash",
            &serde_json::json!({"command": "python scripts/update.py"})
        ),
        "Executing shell command: python scripts/update.py"
    );
    assert_eq!(
        tool_execution_start_progress("grep", &serde_json::json!({"pattern": "Closeout"})),
        "Executing grep..."
    );
}

#[test]
fn test_strip_hidden_blocks_removes_internal_reasoning() {
    let input = "你好<think>内部推理</think>世界";
    assert_eq!(strip_hidden_blocks(input), "你好世界");
}

#[test]
fn test_visible_text_sanitizer_handles_split_think_tags() {
    let mut sanitizer = VisibleTextSanitizer::default();
    let mut out = String::new();
    out.push_str(&sanitizer.push_chunk("你好<th"));
    out.push_str(&sanitizer.push_chunk("ink>不该显示</th"));
    out.push_str(&sanitizer.push_chunk("ink>世界"));
    out.push_str(&sanitizer.finish());
    assert_eq!(out, "你好世界");
}

#[test]
fn test_visible_text_sanitizer_preserves_utf8_chunks_without_panicking() {
    let mut sanitizer = VisibleTextSanitizer::default();
    let mut out = String::new();
    out.push_str(&sanitizer.push_chunk("你"));
    out.push_str(&sanitizer.push_chunk("好"));
    out.push_str(&sanitizer.finish());
    assert_eq!(out, "你好");
}

#[tokio::test]
async fn test_truncate_tool_result_keeps_small_output_unchanged() {
    let original = "short output".to_string();
    let mut result = ToolResult::success(original.clone());
    truncate_tool_result(
        &mut result,
        "grep",
        "call_small",
        None,
        std::path::Path::new("."),
    )
    .await;
    assert_eq!(result.content, original);
}

#[tokio::test]
async fn test_truncate_tool_result_uses_default_tail_preview() {
    let mut result = ToolResult::success(format!(
        "{}\n{}\n{}",
        "A".repeat(40_000),
        "中".repeat(8_000),
        "Z".repeat(40_000)
    ));
    truncate_tool_result(
        &mut result,
        "grep",
        "call_markers",
        None,
        std::path::Path::new("."),
    )
    .await;
    assert!(result.content.contains("--- Last"));
    assert!(result.content.contains("Output truncated"));
    assert_eq!(
        result
            .data
            .as_ref()
            .and_then(|data| data.get("output_truncation"))
            .and_then(|data| data.get("preview_direction"))
            .and_then(|value| value.as_str()),
        Some("Tail")
    );
}

#[test]
fn test_normalize_params_fills_missing_required_fields() {
    let step = crate::engine::plan_mode::PlanStep::new(
        "运行 cargo test 验证修复",
        Some("bash".to_string()),
    );
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "command": { "type": "string" },
            "timeout": { "type": "integer" }
        },
        "required": ["command", "timeout"]
    });

    let out = WorkflowRealStepExecutor::normalize_params(serde_json::json!({}), &schema, &step)
        .expect("normalize should succeed");
    assert_eq!(out["command"], "cargo test");
    assert!(out["timeout"].is_number());
}

#[test]
fn test_normalize_params_coerces_required_field_types() {
    let step = crate::engine::plan_mode::PlanStep::new(
        "在 src/main.rs 中搜索 TODO",
        Some("grep".to_string()),
    );
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string" },
            "path": { "type": "string" },
            "limit": { "type": "integer" },
            "recursive": { "type": "boolean" }
        },
        "required": ["pattern", "path", "limit", "recursive"]
    });

    let out = WorkflowRealStepExecutor::normalize_params(
        serde_json::json!({
            "pattern": 123,
            "path": true,
            "limit": "20",
            "recursive": "yes"
        }),
        &schema,
        &step,
    )
    .expect("normalize should succeed");

    assert_eq!(out["pattern"], "123");
    assert_eq!(out["path"], "true");
    assert_eq!(out["limit"], 20);
    assert_eq!(out["recursive"], true);
}

#[test]
fn test_normalize_params_rejects_non_object_payload() {
    let step =
        crate::engine::plan_mode::PlanStep::new("读取 README.md", Some("file_read".to_string()));
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "path": { "type": "string" } },
        "required": ["path"]
    });
    let err = WorkflowRealStepExecutor::normalize_params(
        serde_json::json!(["not", "object"]),
        &schema,
        &step,
    )
    .expect_err("non-object params should be rejected");
    assert!(err.contains("JSON object"));
}

#[test]
fn test_get_tools_filters_denied_tools_before_model_request() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileReadTool);
    registry.register(BashTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
    .with_session_permission_rules(crate::permissions::PermissionRules::new().deny("bash"));

    let names = loop_instance
        .get_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    assert!(names.contains(&"file_read".to_string()));
    assert!(!names.contains(&"bash".to_string()));
}

#[test]
fn test_get_tools_hides_write_tools_in_read_only_mode() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileReadTool);
    registry.register(FileWriteTool);
    registry.register(BashTool);
    registry.register(GitTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
    .with_permission_mode(crate::permissions::PermissionMode::ReadOnly);

    let names = loop_instance
        .get_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    assert!(names.contains(&"file_read".to_string()));
    assert!(!names.contains(&"file_write".to_string()));
    assert!(!names.contains(&"bash".to_string()));
    assert!(!names.contains(&"git".to_string()));
}

#[test]
fn test_code_action_tools_expose_bash_only_after_changes() {
    let tools = vec![
        crate::services::api::Tool {
            name: "file_edit".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        },
        crate::services::api::Tool {
            name: "file_patch".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        },
        crate::services::api::Tool {
            name: "file_read".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        },
        crate::services::api::Tool {
            name: "grep".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        },
        crate::services::api::Tool {
            name: "bash".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        },
    ];

    let before_change = ConversationLoop::code_action_tools(&tools, false, true)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<HashSet<_>>();
    assert!(before_change.contains("file_edit"));
    assert!(before_change.contains("file_patch"));
    assert!(before_change.contains("file_read"));
    assert!(before_change.contains("grep"));
    assert!(!before_change.contains("bash"));

    let after_change = ConversationLoop::code_action_tools(&tools, true, true)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<HashSet<_>>();
    assert!(after_change.contains("bash"));

    let after_lookup = ConversationLoop::code_action_tools(&tools, false, false)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<HashSet<_>>();
    assert!(after_lookup.contains("file_edit"));
    assert!(after_lookup.contains("file_patch"));
    assert!(!after_lookup.contains("bash"));
    assert!(!after_lookup.contains("file_read"));
    assert!(!after_lookup.contains("grep"));
}

#[test]
fn test_patch_synthesis_is_default_on_with_opt_out() {
    let mut guard = EnvVarGuard::acquire_blocking();
    guard.remove("PRIORITY_AGENT_PATCH_SYNTHESIS");
    assert!(ConversationLoop::patch_synthesis_enabled());

    guard.set("PRIORITY_AGENT_PATCH_SYNTHESIS", "0");
    assert!(!ConversationLoop::patch_synthesis_enabled());
}

#[test]
fn test_verification_source_context_includes_current_error_line() {
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/lib.rs"),
        "fn demo() {\n    let score = 1;\n    let status = missing_value;\n}\n",
    )
    .expect("write source");
    let results = vec![super::super::auto_verify::VerificationResult {
        language: "rust".to_string(),
        command: "cargo check".to_string(),
        success: false,
        issues: vec![super::super::auto_verify::VerificationIssue {
            severity: "error".to_string(),
            file: Some("src/lib.rs".to_string()),
            line: Some(3),
            message: "cannot find value `missing_value` in this scope".to_string(),
        }],
        raw_output: String::new(),
        summary: String::new(),
    }];

    let context = verification_source_context(tmp.path(), &results)
        .expect("verification context should be generated");

    assert!(context.contains("src/lib.rs:3"));
    assert!(context.contains(">    3 |     let status = missing_value;"));
    assert!(context.contains("repair compile/validation errors"));
}

#[test]
fn test_parse_patch_synthesis_plan_from_fenced_json() {
    let content = r#"```json
{"can_patch":true,"reason":"safe","actions":[{"tool":"file_edit","path":"src/lib.rs","old_string":"a","new_string":"b","expected_replacements":1}]}
```"#;
    let plan =
        ConversationLoop::parse_patch_synthesis_plan(content).expect("fenced JSON should parse");
    assert!(plan.can_patch);
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(plan.actions[0].path, "src/lib.rs");
}

#[test]
fn test_patch_synthesis_validation_rejects_parent_traversal() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "../outside.rs".to_string(),
        old_string: Some("a".to_string()),
        new_string: "b".to_string(),
        line_start: None,
        line_end: None,
        expected_replacements: Some(1),
    };
    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("parent traversal must be rejected");
    assert!(err.to_string().contains("parent traversal"));
}

#[test]
fn test_patch_synthesis_line_range_ignores_extra_old_string_for_shell_script() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
    )
    .expect("write script");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: Some("summary_task() {".to_string()),
            new_string: "summary_task() {\n  echo \"# Live Eval Summary: ${RUN_ID}\" >\"$summary\"\n  return 0\n}\n".to_string(),
            line_start: Some(1),
            line_end: Some(4),
            expected_replacements: Some(1),
        };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("line-range shell patch should be accepted");

    assert_eq!(call.arguments["path"], "scripts/run_live_eval.sh");
    assert_eq!(call.arguments["line_start"], 1);
    assert_eq!(call.arguments["line_end"], 4);
    assert!(call.arguments["old_string"].is_null());
}

#[test]
fn test_patch_synthesis_accepts_function_sized_shell_line_range() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    let source = (0..70)
        .map(|idx| format!("  echo line-{idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        format!("summary_task() {{\n{source}\n}}\n"),
    )
    .expect("write script");
    let replacement = (0..70)
        .map(|idx| format!("  printf '%s\\n' item-{idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: None,
        new_string: format!("summary_task() {{\n{replacement}\n}}\n"),
        line_start: Some(1),
        line_end: Some(72),
        expected_replacements: None,
    };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("function-sized shell replacement should be accepted");

    assert_eq!(call.arguments["line_start"], 1);
    assert_eq!(call.arguments["line_end"], 72);
}

#[test]
fn test_patch_synthesis_rejects_shell_line_range_crossing_next_function() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        "summary_task() {\n  echo stub\n}\n\nrun_one() {\n  echo next\n}\n",
    )
    .expect("write script");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: None,
        new_string: "summary_task() {\n  echo ok\n}\n".to_string(),
        line_start: Some(1),
        line_end: Some(6),
        expected_replacements: None,
    };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("cross-function shell replacement should be rejected");

    assert!(err.to_string().contains("crosses function boundary"));
}

#[test]
fn test_patch_synthesis_recovers_shell_function_anchor_from_highlighted_old_string() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n\nrun_one() {\n  echo next\n}\n",
        )
        .expect("write script");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: Some(
            "1359: **summary_task**() {\n  echo \"summary mode is not implemented yet\"\n}"
                .to_string(),
        ),
        new_string: "summary_task() {\n  echo ok\n}\n".to_string(),
        line_start: None,
        line_end: None,
        expected_replacements: Some(1),
    };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("highlighted shell function anchor should recover safely");

    assert!(call.arguments["old_string"]
        .as_str()
        .unwrap_or_default()
        .contains("summary mode is not implemented yet"));
    assert!(!call.arguments["old_string"]
        .as_str()
        .unwrap_or_default()
        .contains("run_one()"));
}

#[test]
fn test_patch_synthesis_rejects_bare_live_eval_parser_import_in_shell_heredoc() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
    )
    .expect("write script");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: None,
        new_string: r#"summary_task() {
python3 - <<'PY'
import pathlib
import sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from live_eval_report_parser import report_rows
PY
}
"#
        .to_string(),
        line_start: Some(1),
        line_end: Some(4),
        expected_replacements: Some(1),
    };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("bare live_eval_report_parser import should be rejected");

    assert!(err
        .to_string()
        .contains("Python heredocs execute from stdin"));
}

#[test]
fn test_patch_synthesis_rejects_markdown_highlight_in_shell_patch() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
    )
    .expect("write script");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: None,
        new_string: "**summary_task()** {\n  echo ok\n}\n".to_string(),
        line_start: Some(1),
        line_end: Some(4),
        expected_replacements: Some(1),
    };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("markdown highlighting should be rejected");

    assert!(err.to_string().contains("Markdown emphasis markers"));
}

#[test]
fn test_patch_synthesis_accepts_scripts_package_import_in_shell_heredoc() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
    )
    .expect("write script");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "scripts/run_live_eval.sh".to_string(),
        old_string: None,
        new_string: r#"summary_task() {
python3 - <<'PY'
from scripts.live_eval_report_parser import report_rows
PY
}
"#
        .to_string(),
        line_start: Some(1),
        line_end: Some(4),
        expected_replacements: Some(1),
    };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("package import should be accepted");

    assert_eq!(call.arguments["path"], "scripts/run_live_eval.sh");
}

#[test]
fn test_patch_synthesis_path_resolves_root_relative_src_path() {
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(tmp.path().join("src/lib.rs"), "fn main() {}\n").expect("write file");

    let (canonical, tool_path) = ConversationLoop::resolve_synthesized_patch_path(
        std::path::Path::new("/src/lib.rs"),
        tmp.path(),
    )
    .expect("root-relative src path should resolve inside cwd");

    assert_eq!(
        canonical,
        tmp.path().join("src/lib.rs").canonicalize().unwrap()
    );
    assert_eq!(tool_path, "src/lib.rs");
}

#[test]
fn test_patch_synthesis_recovers_wrong_path_from_unique_old_string() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/assessment.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("unique old_string should recover the real file path");

    assert_eq!(call.arguments["path"], "src/memory/quality.rs");
}

#[test]
fn test_patch_synthesis_keeps_failed_compiler_evidence() {
    let messages = vec![Message::tool(
            "cargo_check",
            "Result: ERROR\nerror[E0596]: cannot borrow `self.memory_manager.0` as mutable\n[exit status: 101]",
        )];

    let evidence = ConversationLoop::patch_synthesis_evidence(&messages);

    assert!(evidence.contains("FAILED TOOL RESULT"));
    assert!(evidence.contains("error[E0596]"));
}

#[test]
fn test_patch_synthesis_large_file_evidence_keeps_relevant_late_function() {
    let mut content = String::from("Result: OK\n");
    for idx in 0..600 {
        content.push_str(&format!("{idx:4} | echo filler_{idx}\n"));
    }
    content.push_str(
            "1359 | summary_task() {\n1360 |   echo \"summary mode is not implemented yet\" >&2\n1361 |   return 2\n1362 | }\n",
        );
    for idx in 601..900 {
        content.push_str(&format!("{idx:4} | echo tail_{idx}\n"));
    }
    let messages = vec![Message::tool("file_read", content)];

    let evidence = ConversationLoop::patch_synthesis_evidence(&messages);

    assert!(evidence.contains("summary_task()"));
    assert!(evidence.contains("summary mode is not implemented yet"));
    assert!(evidence.contains("[relevant excerpt]"));
}

#[test]
fn test_deterministic_patch_synthesis_repairs_ref_mut_e0596() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "if let Some(ref mut mem_mutex) = self.memory_manager {\n    let mut mem = mem_mutex.lock().await;\n}\n",
        )
        .expect("write module file");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0596]: cannot borrow `self.memory_manager.0` as mutable, as it is behind a `&` reference",
            tmp.path(),
        );

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments["old_string"],
        "if let Some(ref mut mem_mutex) = self.memory_manager {"
    );
    assert_eq!(
        calls[0].arguments["new_string"],
        "if let Some(ref mem_mutex) = self.memory_manager {"
    );
}

#[test]
fn test_deterministic_patch_synthesis_scaffolds_local_web_mvp() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileWriteTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("fixtures/project_partner_vague_tool"))
        .expect("create fixture dir");
    std::fs::write(
        tmp.path()
            .join("fixtures/project_partner_vague_tool/README.md"),
        "tiny local tool for lab strains and phage notes; local-only",
    )
    .expect("write readme");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "Build the smallest useful local web MVP under fixtures/project_partner_vague_tool. Missing index.html. It must mention strain, phage, and localStorage while staying local-only.",
            tmp.path(),
        );

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "file_write");
    assert_eq!(
        calls[0].arguments["path"],
        "fixtures/project_partner_vague_tool/index.html"
    );
    let content = calls[0].arguments["content"]
        .as_str()
        .expect("file_write content");
    assert!(content.contains("localStorage"));
    assert!(content.contains("Strain"));
    assert!(content.contains("Phage"));
}

#[test]
fn test_deterministic_patch_fallback_records_source_and_reason() {
    let mut guard = EnvVarGuard::acquire_blocking();
    guard.remove("PRIORITY_AGENT_DETERMINISTIC_PATCH_SYNTHESIS");
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "if let Some(ref mut mem_mutex) = self.memory_manager {\n    let mut mem = mem_mutex.lock().await;\n}\n",
        )
        .expect("write module file");

    let outcome = loop_instance
            .deterministic_patch_fallback(
                "error[E0596]: cannot borrow `self.memory_manager.0` as mutable, as it is behind a `&` reference",
                tmp.path(),
                "model patch synthesis failed: invalid JSON",
            )
            .expect("deterministic fallback should produce a repair");

    assert_eq!(
        outcome.source,
        super::patch_recovery::PatchSynthesisSource::DeterministicFallback
    );
    assert_eq!(
        outcome.fallback_reason.as_deref(),
        Some("model patch synthesis failed: invalid JSON")
    );
    assert_eq!(outcome.tool_calls.len(), 1);
    assert_eq!(outcome.tool_calls[0].name, "file_edit");
}

#[test]
fn test_deterministic_patch_synthesis_repairs_persistent_memory_marker() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
            tmp.path()
                .join("src/engine/conversation_loop/turn_retrieval_context_controller.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n\n        if let Some(ref ctx) = turn_retrieval_context {\n",
        )
        .expect("write module file");

    let calls = loop_instance.deterministic_patch_tool_calls(
        "the regression marker identifies the missing planning prefetch block",
        tmp.path(),
    );

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments["path"],
        "src/engine/conversation_loop/turn_retrieval_context_controller.rs"
    );
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("Self::build_memory_context(&context).await"));
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("Self::merge_context(&mut turn_retrieval_context, memory_ctx)"));
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("Self::record_memory_prefetch(context.trace, &memory_ctx)"));
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("if let Some(ref ctx) = turn_retrieval_context"));
    assert!(!calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("futures::executor::block_on"));
}

#[test]
fn test_deterministic_patch_synthesis_repairs_persistent_memory_context_borrow() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
        tmp.path()
            .join("src/engine/conversation_loop/turn_retrieval_context_controller.rs"),
        "if let Some(memory_ctx) = Self::build_memory_context(context).await {}\n",
    )
    .expect("write module file");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0308]: mismatched types expected `&TurnRetrievalContextRequest<'_>`, found `TurnRetrievalContextRequest<'_>` at build_memory_context(context)",
            tmp.path(),
        );

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments["path"],
        "src/engine/conversation_loop/turn_retrieval_context_controller.rs"
    );
    assert_eq!(
        calls[0].arguments["new_string"],
        "Self::build_memory_context(&context).await"
    );
}

#[test]
fn test_deterministic_patch_synthesis_repairs_live_eval_summary_stub() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
    std::fs::write(
        tmp.path().join("scripts/run_live_eval.sh"),
        r###"summary_task() {
  local run_report_dir="$REPORT_DIR/live-$RUN_ID"
  local summary="$run_report_dir/summary.md"
  mkdir -p "$run_report_dir"
  echo "summary mode is not implemented yet" >&2
  echo "# Live Eval Summary: $RUN_ID" >"$summary"
  echo "" >>"$summary"
  echo "- status: not_implemented" >>"$summary"
  return 2
}

run_one() {
  echo next
}
"###,
    )
    .expect("write live eval script");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "TASK: live-eval-dashboard-summary requires summary_task to generate plan_quality, tool_boundary, and verification_status",
            tmp.path(),
        );

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments["path"], "scripts/run_live_eval.sh");
    assert_eq!(calls[0].arguments["line_start"], 1);
    assert_eq!(calls[0].arguments["line_end"], 10);
    let replacement = calls[0].arguments["new_string"].as_str().unwrap();
    assert!(replacement.contains("from scripts.live_eval_report_parser import report_rows"));
    assert!(replacement.contains("plan_quality"));
    assert!(replacement.contains("tool_boundary"));
    assert!(replacement.contains("verification_status"));
    assert!(!replacement.contains("summary mode is not implemented yet"));
}

#[test]
fn test_deterministic_patch_synthesis_repairs_record_repair_action_arity() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    let damaged_call = concat!(
        r#"fn repair() {
                if !verify_passed {
                    let verification_command = failed_commands
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "post-edit verification".to_string());
                    post_edit_reflection.record_repair_action(
                  acceptance_repair_attempts + 1,
                  &format!("retry: {"#,
        r#"}", verification_command),
                  changed_files.first().map(|path| path.display().to_string()),
              );
                }
}
"#
    );
    std::fs::write(
        tmp.path()
            .join("src/engine/conversation_loop/repair_controller.rs"),
        damaged_call,
    )
    .expect("write repair controller file");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0061]: this method takes 4 arguments but 3 arguments were supplied\nargument #4 is missing\nrecord_repair_action",
            tmp.path(),
        );

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments["path"],
        "src/engine/conversation_loop/repair_controller.rs"
    );
    assert_eq!(calls[0].arguments["line_start"], 7);
    assert_eq!(calls[0].arguments["line_end"], 11);
    let replacement = calls[0].arguments["new_string"].as_str().unwrap();
    assert!(replacement.contains("context.acceptance_repair_attempts + 1"));
    assert!(replacement.contains("\"repair failed verification before closeout\""));
    assert!(replacement.contains("verification_command,"));
    assert!(!replacement.contains(ConversationLoop::retry_format_marker().as_str()));
}

#[test]
fn test_deterministic_patch_synthesis_repairs_skill_promotion_gate_apply_path() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
        .expect("create slash handler dir");
    std::fs::write(
        tmp.path().join("src/tui/slash_handler/learning.rs"),
        r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
    )
    .expect("write fixture file");

    let calls = loop_instance.deterministic_patch_tool_calls(
            "skill-promotion-gate required command failed because validate_skill_promotion_for_apply is not called before write_active_skill and EvolutionController cooldown is missing",
            tmp.path(),
        );

    assert_eq!(calls.len(), 2);
    let first = calls[0].arguments["new_string"].as_str().unwrap();
    assert!(first
        .contains("validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"));
    assert!(first.contains("Skill proposal {} was not applied by promotion gate"));
    let second = calls[1].arguments["new_string"].as_str().unwrap();
    assert!(second.contains("record_evolution_update("));
    assert!(second.contains("EvolutionTarget::Skill"));
}

#[test]
fn test_deterministic_patch_synthesis_uses_skill_task_preview_without_failed_evidence() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
        .expect("create slash handler dir");
    std::fs::write(
        tmp.path().join("src/tui/slash_handler/learning.rs"),
        r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
    )
    .expect("write fixture file");

    let task_seed =
        "TASK:\n修复 /skill-proposals apply 没有强制使用 fitness promotion gate 的问题。";
    let calls = loop_instance.deterministic_patch_tool_calls(task_seed, tmp.path());

    assert_eq!(calls.len(), 2);
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"));
    assert!(calls[1].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("record_evolution_update("));
}

#[test]
fn test_deterministic_patch_synthesis_ignores_unrelated_memory_tool_mentions() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/tools/memory_tool"))
        .expect("create memory tool dir");
    std::fs::write(
        tmp.path().join("src/tools/memory_tool/mod.rs"),
        "let assessment = assess_memory_candidate(content, category, &existing, true);\n",
    )
    .expect("write fixture file");

    let calls = loop_instance.deterministic_patch_tool_calls(
        "resume-session-picker inspected /resume and saw memory_save while checking whether \
             restore_session flushes current memory before switching sessions",
        tmp.path(),
    );

    assert!(
        calls.is_empty(),
        "memory quality repair must not fire for unrelated resume tasks"
    );
}

#[test]
fn test_deterministic_patch_synthesis_repairs_memory_quality_gate() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/tools/memory_tool"))
        .expect("create memory tool dir");
    std::fs::write(
        tmp.path().join("src/tools/memory_tool/mod.rs"),
        "let assessment = assess_memory_candidate(content, category, &existing, true);\n",
    )
    .expect("write fixture file");

    let calls = loop_instance.deterministic_patch_tool_calls(
        "memory-save-quality-gate found that explicit memory_save bypasses the quality gate",
        tmp.path(),
    );

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments["path"], "src/tools/memory_tool/mod.rs");
    assert_eq!(
        calls[0].arguments["new_string"],
        "assess_memory_candidate(content, category, &existing, false)"
    );
}

#[test]
fn test_deterministic_patch_synthesis_repairs_explicit_proposed_memory_status() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
        tmp.path().join("src/memory/quality.rs"),
        r#"let status = if score >= 0.65 {
        MemoryStatus::Accepted
    } else if explicit && score >= 0.45 {
        // Explicit override lowers threshold but still respects hard limits from score_memory_write
        MemoryStatus::Proposed
    } else {
        write_decision.status
    };
"#,
    )
    .expect("write fixture file");

    let calls = loop_instance.deterministic_patch_tool_calls(
        "memory-save-quality-gate still allows explicit save to bypass the quality gate",
        tmp.path(),
    );

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments["path"], "src/memory/quality.rs");
    assert_eq!(
        calls[0].arguments["new_string"],
        "let status = write_decision.status;"
    );
}

#[test]
fn test_patch_synthesis_prefers_deterministic_rule_before_model_json() {
    static CWD_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    let _cwd_guard = CWD_LOCK.get_or_init(|| StdMutex::new(())).lock().unwrap();
    let original_cwd = std::env::current_dir().expect("read current dir");
    struct CurrentDirGuard(std::path::PathBuf);
    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _restore = CurrentDirGuard(original_cwd);

    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/tools/memory_tool"))
        .expect("create memory tool dir");
    std::fs::write(
        tmp.path().join("src/tools/memory_tool/mod.rs"),
        "let assessment = assess_memory_candidate(content, category, &existing, true);\n",
    )
    .expect("write fixture file");
    std::env::set_current_dir(tmp.path()).expect("switch current dir");

    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    let outcome = runtime
            .block_on(loop_instance.synthesize_patch_tool_calls(
                &[Message::user(
                    "memory-save-quality-gate found that explicit memory_save bypasses the quality gate",
                )],
                "memory-save-quality-gate",
            ))
            .expect("deterministic patch synthesis outcome");

    assert_eq!(
        outcome.source,
        super::patch_recovery::PatchSynthesisSource::DeterministicFallback
    );
    assert_eq!(
        outcome.fallback_reason.as_deref(),
        Some("deterministic patch repair rule matched before model synthesis")
    );
    assert_eq!(outcome.tool_calls.len(), 1);
    assert_eq!(
        outcome.tool_calls[0].arguments["path"],
        "src/tools/memory_tool/mod.rs"
    );
}

#[test]
fn test_deterministic_patch_synthesis_repairs_memory_recall_conflict_precision() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine")).expect("create engine dir");
    std::fs::write(
        tmp.path().join("src/engine/retrieval_context.rs"),
        r#"fn memory_conflict_matches_item(
    conflict: &str,
    item: &crate::memory::manager::MemoryMatch,
) -> bool {
    let conflict = conflict.to_lowercase();
    let snippet = item.snippet.to_lowercase();
    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !matches!(
                    *part,
                    "memory" | "project" | "user" | "value" | "values" | "conflicting"
                )
        })
        .collect::<Vec<_>>();
    tokens.len() >= 2
        && tokens
            .iter()
            .filter(|part| snippet.contains(**part))
            .count()
            >= 2
}

fn parse_memory_conflict(conflict: &str) -> Option<(String, Vec<String>)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_conflict_matching_uses_structured_key_and_value() {
        let conflict = "- key 'language' has conflicting values: chinese | english";
        let unrelated = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "The project memory mentions conflicting work before.".to_string(),
        };
        let related = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "language: Chinese\nUse compact CLI status bars.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {}
}
"#,
    )
    .expect("write fixture file");

    let calls = loop_instance.deterministic_patch_tool_calls(
        "TASK:\n强化记忆检索中的冲突匹配精度。memory-recall-conflict-precision",
        tmp.path(),
    );

    assert_eq!(calls.len(), 3);
    assert!(calls[0].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("is_generic_conflict_token(&key)"));
    assert!(calls[1].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("fn is_generic_conflict_token("));
    assert!(calls[2].arguments["new_string"]
        .as_str()
        .unwrap()
        .contains("memory_conflict_matching_ignores_generic_key_conflicts"));
}

#[test]
fn test_patch_synthesis_rejects_bad_persistent_memory_async_shape() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(memory_ctx) = self
            .memory_manager
            .as_mut()
            .and_then(|m| {
                futures::executor::block_on(m.prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    self.provider.as_ref().and_then(|p| p.preferred_model()).unwrap_or("default"),
                    route.retrieval,
                ))
            })
        {
            turn_retrieval_context = Some(memory_ctx);
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("bad async memory block should be rejected")
        .to_string();

    assert!(err.contains("block_on"));
}

#[test]
fn test_patch_synthesis_rejects_provider_option_style_in_memory_prefetch() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
        .expect("create module dir");
    std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            if let Some(mem_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref().map(|p| p.as_ref()).unwrap(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                turn_retrieval_context = Some(mem_ctx);
            }
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("provider option-style call should be rejected")
        .to_string();

    assert!(err.contains("Option"));
}

#[test]
fn test_validation_tool_call_detects_success_gate_commands() {
    let cargo_test = ToolCall {
        id: "test".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "cargo test -q -- --test-threads=1"
        }),
    };
    let ls = ToolCall {
        id: "ls".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "ls -la"
        }),
    };
    let file_read = ToolCall {
        id: "read".to_string(),
        name: "file_read".to_string(),
        arguments: serde_json::json!({
            "path": "src/main.rs"
        }),
    };
    let python_assertion = ToolCall {
        id: "python".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "python3 -c \"assert True\""
        }),
    };
    let node_test = ToolCall {
        id: "node".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "node fixtures/live_frontend/book_notes/test-book-notes.cjs"
        }),
    };
    let python_unittest = ToolCall {
        id: "unittest".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py"
        }),
    };
    let rg_assertion = ToolCall {
        id: "rg".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "! rg 'bad_pattern' src/lib.rs"
        }),
    };
    let rg_assertion_with_ampersand_pattern = ToolCall {
        id: "rg_amp".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs"
        }),
    };
    let env_prefixed_cargo_test = ToolCall {
        id: "env_test".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        }),
    };
    let shell_wrapped_cargo_test = ToolCall {
        id: "wrapped_test".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
        }),
    };

    assert!(RequiredValidationController::is_validation_tool_call(
        &cargo_test
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &python_assertion
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &node_test
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &python_unittest
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &rg_assertion
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &rg_assertion_with_ampersand_pattern
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &env_prefixed_cargo_test
    ));
    assert!(RequiredValidationController::is_validation_tool_call(
        &shell_wrapped_cargo_test
    ));
    assert!(!RequiredValidationController::is_validation_tool_call(&ls));
    assert!(!RequiredValidationController::is_validation_tool_call(
        &file_read
    ));
}

#[test]
fn test_validation_command_match_normalizes_shell_lc_wrappers() {
    assert_eq!(
            RequiredValidationController::normalize_command_for_match(
                "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            ),
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        );
    assert_eq!(
        RequiredValidationController::normalize_command_for_match(
            "  env   PRIORITY_AGENT_WORKFLOW_ENABLED=1   cargo test --quiet -- --test-threads=1  "
        ),
        "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
    );
}

#[test]
fn test_required_validation_pending_commands_normalizes_already_run() {
    let required = vec![
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
                .to_string(),
            "rg '^cleanup = skipped by user request$' fixtures/core_quality/permission_rejection/manifest.txt"
                .to_string(),
        ];
    let successful_validation = vec![
        "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            .to_string(),
    ];
    let successful_required = HashSet::new();

    assert_eq!(
            RequiredValidationController::pending_commands(
                &required,
                &successful_validation,
                &successful_required,
            ),
            vec![
                "rg '^cleanup = skipped by user request$' fixtures/core_quality/permission_rejection/manifest.txt"
                    .to_string()
            ]
        );
}

#[test]
fn test_successful_validation_command_matches_required_command() {
    let required = vec![
        "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1".to_string(),
    ];
    let tool_call = ToolCall {
        id: "wrapped_test".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
        }),
    };

    let command = RequiredValidationController::successful_validation_command(&tool_call, true)
        .expect("successful validation command");

    assert!(RequiredValidationController::command_matches_required(
        &required, &command
    ));
    assert!(
        RequiredValidationController::successful_validation_command(&tool_call, false).is_none()
    );
}

#[test]
fn test_required_validation_summary_partitions_failed_results() {
    let outcome = RequiredValidationController::summarize_results(vec![
        super::super::auto_verify::VerificationResult {
            language: "required".to_string(),
            command: "test -f keep.txt".to_string(),
            success: true,
            issues: Vec::new(),
            raw_output: String::new(),
            summary: "required command passed: test -f keep.txt".to_string(),
        },
        super::super::auto_verify::VerificationResult {
            language: "required".to_string(),
            command: "rg '^status = corrected$' manifest.txt".to_string(),
            success: false,
            issues: vec![super::super::auto_verify::VerificationIssue {
                severity: "error".to_string(),
                file: None,
                line: None,
                message: "not found".to_string(),
            }],
            raw_output: String::new(),
            summary: "required command failed: rg '^status = corrected$' manifest.txt".to_string(),
        },
    ]);

    assert!(!outcome.passed);
    assert_eq!(outcome.items.len(), 2);
    assert!(outcome.items[0].success);
    assert!(!outcome.items[1].success);
    assert!(outcome.items[1].dialog_text.contains("not found"));

    let application = RequiredValidationController::application_for_run(outcome);
    assert!(!application.passed);
    assert_eq!(application.acceptance_evidence.len(), 2);
    assert_eq!(
        application.successful_commands,
        vec!["test -f keep.txt".to_string()]
    );
    assert_eq!(
        application.failed_commands,
        vec!["rg '^status = corrected$' manifest.txt".to_string()]
    );
    assert_eq!(application.post_edit_evidence.len(), 1);
    assert_eq!(application.ledger_records.len(), 2);
    assert!(!application.ledger_records[1].success);
}

#[test]
fn test_extract_required_validation_commands_from_live_eval_prompt() {
    let prompt = r#"
## Acceptance checks
- `env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1`
- `cargo test -q learning_planning -- --test-threads=1`
- `node fixtures/live_frontend/book_notes/test-book-notes.cjs`
- `python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py`
- `python3 -c "p='src/lib.rs'; assert True"`
- `! rg 'bad_pattern' src/lib.rs`
- `! rg '&format!\("retry: \{\}", verification_command\)' src/engine/conversation_loop/mod.rs`
- `rg 'good_pattern' src/lib.rs`
- `rg '^cleanup = skipped by user request$' fixtures/core_quality/permission_rejection/manifest.txt`
- `. .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'`
- `rm -rf /tmp/nope`
- `(none)`
"#;

    let commands = RequiredValidationController::extract_commands(prompt);

    assert_eq!(
            commands,
            vec![
                "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1".to_string(),
                "cargo test -q learning_planning -- --test-threads=1".to_string(),
                "node fixtures/live_frontend/book_notes/test-book-notes.cjs".to_string(),
                "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py".to_string(),
                "python3 -c \"p='src/lib.rs'; assert True\"".to_string(),
                "! rg 'bad_pattern' src/lib.rs".to_string(),
                "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs".to_string(),
                "rg 'good_pattern' src/lib.rs".to_string(),
                "rg '^cleanup = skipped by user request$' fixtures/core_quality/permission_rejection/manifest.txt".to_string(),
                ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'".to_string(),
            ]
        );
}

#[test]
fn test_required_validation_disables_default_auto_tests() {
    assert!(RequiredValidationController::should_run_default_auto_tests(
        &[]
    ));
    assert!(
        !RequiredValidationController::should_run_default_auto_tests(&[
            "cargo test -q -- --test-threads=1".to_string()
        ])
    );
}

#[test]
fn test_patch_synthesis_recovers_assignment_anchor_when_old_string_is_inexact() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "src/memory/quality.rs".to_string(),
        old_string: Some(
            "let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };"
                .to_string(),
        ),
        new_string: "let status = write_decision.status;".to_string(),
        line_start: None,
        line_end: None,
        expected_replacements: Some(1),
    };

    let call = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect("unique assignment anchor should recover exact old_string");

    assert_eq!(
            call.arguments["old_string"],
            "    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
        );
    assert_eq!(
        call.arguments["new_string"],
        "    let status = write_decision.status;"
    );
}

#[test]
fn test_patch_synthesis_rejects_inexact_multiline_replacement() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {\n    MemoryStatus::Accepted\n} else {\n    write_decision.status\n};".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("inexact multiline replacement should be rejected");
    assert!(err.to_string().contains("inexact multi-line replacement"));
}

#[test]
fn test_patch_synthesis_rejects_unbalanced_replacement() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("unbalanced replacement should be rejected");
    assert!(err.to_string().contains("unbalanced delimiters"));
}

#[test]
fn test_patch_synthesis_rejects_score_based_memory_status_promotion() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("score-only accepted promotion should be rejected");
    assert!(err
        .to_string()
        .contains("preserve score_memory_write hard gates"));
}

#[test]
fn test_patch_synthesis_rejects_unknown_enum_variant() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/types.rs"),
        "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n",
    )
    .expect("write types");
    std::fs::write(
        tmp.path().join("src/quality.rs"),
        "let status = MemoryStatus::Accepted;\n",
    )
    .expect("write quality");
    let action = PatchSynthesisAction {
        tool: "file_edit".to_string(),
        path: "src/quality.rs".to_string(),
        old_string: Some("let status = MemoryStatus::Accepted;".to_string()),
        new_string: "let status = MemoryStatus::Blocked;".to_string(),
        line_start: None,
        line_end: None,
        expected_replacements: Some(1),
    };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("unknown enum variant should be rejected before editing");

    assert!(err.to_string().contains("MemoryStatus::Blocked"));
    assert!(err.to_string().contains("Accepted"));
}

#[test]
fn test_patch_synthesis_rejects_memory_status_duplicate_extension() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileEditTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let tmp = tempdir().expect("create temp dir");
    std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
    let old_enum = "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n";
    std::fs::write(tmp.path().join("src/memory/types.rs"), old_enum).expect("write types");
    let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/types.rs".to_string(),
            old_string: Some(old_enum.to_string()),
            new_string: "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n    Duplicate,\n    Demoted,\n}\n".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

    let err = loop_instance
        .validate_patch_synthesis_action(&action, tmp.path())
        .expect_err("duplicate/demote should use MemoryWriteOutcomeStatus");

    assert!(err.to_string().contains("MemoryWriteOutcomeStatus"));
}

#[tokio::test]
async fn test_tool_specific_confirmation_blocks_git_push_without_approval() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(GitTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
    let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
    let destructive_scope =
        crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
            "push the branch",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
    let tool_calls = vec![ToolCall {
        id: "git_push".to_string(),
        name: "git".to_string(),
        arguments: serde_json::json!({"action": "push"}),
    }];
    let exposed_tool_names = HashSet::from(["git".to_string()]);
    let mut lifecycle = tool_call_lifecycle::ToolCallLifecycle::default();
    let mut storm_state = crate::engine::repair::storm::StormState::default();

    let batch =
        ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls: &tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed: Default::default(),
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                task_stage: crate::engine::task_context::AgentTaskStage::Repair,
                task_state: None,
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                storm_state: &mut storm_state,
                lifecycle: &mut lifecycle,
            })
            .await;
    let results = batch.results();

    assert_eq!(results.len(), 1);
    assert!(!results[0].1.success);
    assert!(results[0]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("requires user confirmation"));
    assert_eq!(
        results[0].1.data.as_ref().unwrap()["permission_request"]["kind"],
        "runtime_rule"
    );
    assert_eq!(
        results[0].1.data.as_ref().unwrap()["permission_request"]["metadata"]["tool_name"],
        "git"
    );
}

#[tokio::test]
async fn test_unexposed_tool_call_is_denied_before_execution() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(GitTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
    let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
    let destructive_scope =
        crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
            "push the branch",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
    let tool_calls = vec![ToolCall {
        id: "git_push".to_string(),
        name: "git".to_string(),
        arguments: serde_json::json!({"action": "push"}),
    }];
    let exposed_tool_names = HashSet::from(["file_edit".to_string()]);
    let mut lifecycle = tool_call_lifecycle::ToolCallLifecycle::default();
    let retained_context =
        crate::tools::ToolContextRetainedContext::default().with_skill_triggers(vec![
            crate::tools::ToolContextSkillTrigger {
                name: "repo-review".to_string(),
                description: "Review repo changes".to_string(),
                triggers: vec!["push".to_string()],
                allowed_tools: vec!["git".to_string()],
                disallowed_tools: Vec::new(),
                model: None,
                effort: None,
                context: Some("inherit".to_string()),
                provenance: "test.skill".to_string(),
            },
        ]);

    let mut storm_state = crate::engine::repair::storm::StormState::default();
    let batch =
        ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls: &tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed: Default::default(),
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &retained_context,
                task_stage: crate::engine::task_context::AgentTaskStage::Understand,
                task_state: None,
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                storm_state: &mut storm_state,
                lifecycle: &mut lifecycle,
            })
            .await;
    let results = batch.results();

    assert_eq!(results.len(), 1);
    assert!(!results[0].1.success);
    assert!(results[0]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("was not exposed"));
    assert_eq!(
        results[0].1.data.as_ref().unwrap()["action_review"]["decision"],
        "revise"
    );
    assert_eq!(
        results[0].1.data.as_ref().unwrap()["action_review"]["primary_reason"],
        "tool_not_exposed"
    );
    let runtime = &results[0].1.data.as_ref().unwrap()["tool_runtime"];
    assert!(runtime["route"]["intent"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(
        runtime["policy"]["max_tool_calls"].as_u64(),
        Some(policy.max_tool_calls as u64)
    );
    assert_eq!(
        runtime["execution"]["exposed_tools_count"].as_u64(),
        Some(exposed_tool_names.len() as u64)
    );
    assert_eq!(
        runtime["execution"]["action_checkpoint_active"].as_bool(),
        Some(false)
    );
    assert_eq!(
        runtime["retained_context"]["skill_triggers"].as_u64(),
        Some(1)
    );
    assert_eq!(
        runtime["retained_context"]["retrieval_items"].as_u64(),
        Some(0)
    );
    let started_at = runtime["execution"]["started_at_unix_ms"]
        .as_u64()
        .expect("tool runtime should include start timestamp");
    let finished_at = runtime["execution"]["finished_at_unix_ms"]
        .as_u64()
        .expect("tool runtime should include finish timestamp");
    assert!(started_at <= finished_at);
}

#[tokio::test]
async fn invalid_tool_params_are_rejected_before_execution() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let route = crate::engine::intent_router::IntentRouter::new().route("run a command");
    let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
    let destructive_scope =
        crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
            "run a command",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
    let tool_calls = vec![ToolCall {
        id: "bash_missing_command".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({}),
    }];
    let exposed_tool_names = HashSet::from(["bash".to_string()]);
    let mut lifecycle = tool_call_lifecycle::ToolCallLifecycle::default();
    let mut storm_state = crate::engine::repair::storm::StormState::default();

    let batch =
        ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls: &tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed: Default::default(),
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                task_stage: crate::engine::task_context::AgentTaskStage::Understand,
                task_state: None,
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                storm_state: &mut storm_state,
                lifecycle: &mut lifecycle,
            })
            .await;
    let results = batch.results();

    assert_eq!(results.len(), 1);
    assert!(!results[0].1.success);
    assert_eq!(
        results[0].1.error_code,
        Some(crate::tools::ToolErrorCode::InvalidParams)
    );
    assert!(results[0]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("Missing required parameter: command"));
    assert_eq!(
        results[0].1.data.as_ref().unwrap()["schema_validation"]["valid"],
        false
    );
}

#[tokio::test]
async fn destructive_scope_blocks_parent_delete_before_bash_execution() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let route = crate::engine::intent_router::IntentRouter::new().route("删除 abc.txt");
    let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
    let destructive_scope =
        crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
            "删除 abc.txt",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
    let tool_calls = vec![ToolCall {
        id: "rm_parent".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({"command": "rm -rf /tmp/gex"}),
    }];
    let exposed_tool_names = HashSet::from(["bash".to_string()]);
    let mut lifecycle = tool_call_lifecycle::ToolCallLifecycle::default();
    let mut storm_state = crate::engine::repair::storm::StormState::default();

    let batch =
        ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls: &tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed: Default::default(),
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                task_stage: crate::engine::task_context::AgentTaskStage::Repair,
                task_state: None,
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                storm_state: &mut storm_state,
                lifecycle: &mut lifecycle,
            })
            .await;
    let results = batch.results();

    assert_eq!(results.len(), 1);
    assert!(!results[0].1.success);
    assert!(results[0]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("Destructive scope blocked"));
}

struct MockLlmProvider {
    responses: StdMutex<VecDeque<ChatResponse>>,
}

#[async_trait::async_trait]
impl LlmProvider for MockLlmProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        let mut guard = self.responses.lock().unwrap();
        guard
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no mock response left"))
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        Err(anyhow::anyhow!("stream not used in this test"))
    }

    fn base_url(&self) -> &str {
        "mock://local"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

#[tokio::test]
async fn runtime_diet_report_is_recorded_for_real_loop_turn() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::from(vec![ChatResponse {
            content: "hello".to_string(),
            tool_calls: None,
            usage: Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 3,
                total_tokens: 15,
                reasoning_tokens: None,
                cached_tokens: None,
            }),
            tool_call_repair: None,
        }])),
    });
    let tool_registry = Arc::new(ToolRegistry::new());
    let cost_tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));
    let trace_store = Arc::new(TraceStore::default());
    let loop_instance = ConversationLoop::new(provider, tool_registry, cost_tracker, "test".into())
        .with_trace_store(trace_store.clone())
        .with_max_iterations(1);

    let result = loop_instance
        .run(vec![Message::user("请简单回复一句 hello")])
        .await
        .expect("loop should complete");

    assert_eq!(result.content, "hello");
    let trace = trace_store.latest().expect("trace should be recorded");
    let diet = trace.events.iter().find_map(|event| {
        if let TraceEvent::RuntimeDietReport {
            prompt_tokens,
            tool_schema_tokens,
            exposed_tools,
            memory_snapshot_tokens,
            retrieval_items,
            skill_list_tokens,
            workflow_context,
            validation_evidence,
            ..
        } = event
        {
            Some((
                *prompt_tokens,
                *tool_schema_tokens,
                *exposed_tools,
                *memory_snapshot_tokens,
                *retrieval_items,
                *skill_list_tokens,
                workflow_context.as_str(),
                validation_evidence.as_str(),
            ))
        } else {
            None
        }
    });
    let (
        prompt_tokens,
        tool_schema_tokens,
        exposed_tools,
        memory_snapshot_tokens,
        retrieval_items,
        skill_list_tokens,
        workflow_context,
        validation,
    ) = diet.expect("runtime diet event should be recorded");
    assert!(prompt_tokens > 0);
    assert_eq!(tool_schema_tokens, 0);
    assert_eq!(exposed_tools, 0);
    assert_eq!(memory_snapshot_tokens, 0);
    assert_eq!(retrieval_items, 0);
    assert_eq!(skill_list_tokens, 0);
    assert_eq!(workflow_context, "none");
    assert_eq!(validation, "none");
    assert!(crate::engine::trace::format_trace_summary(&trace, 80).contains("Runtime Diet:"));
}

struct CapturingLlmProvider {
    requests: StdMutex<Vec<ChatRequest>>,
    response: StdMutex<Option<ChatResponse>>,
}

#[async_trait::async_trait]
impl LlmProvider for CapturingLlmProvider {
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
        self.requests.lock().unwrap().push(request);
        self.response
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| anyhow::anyhow!("no mock response left"))
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        Err(anyhow::anyhow!("stream not used in this test"))
    }

    fn base_url(&self) -> &str {
        "mock://local"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

#[tokio::test]
async fn quiet_direct_turn_stays_in_main_loop_without_tools_or_dynamic_context() {
    let provider = Arc::new(CapturingLlmProvider {
        requests: StdMutex::new(Vec::new()),
        response: StdMutex::new(Some(ChatResponse {
            content: "你好，我在。".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        })),
    });
    let (tx, mut rx) = mpsc::channel(8);
    let loop_instance = ConversationLoop::new(
        provider.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
    .with_max_iterations(5);

    let result = loop_instance
        .run_streaming(vec![Message::user("你好")], &tx)
        .await
        .expect("loop should complete");

    assert_eq!(result.content, "你好，我在。");
    assert_eq!(result.iterations, 1);
    assert!(!result.tool_calls_made);

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].tools.as_ref().map(Vec::len), Some(0));
    assert!(requests[0].tool_choice.is_none());
    assert!(requests[0].messages.iter().all(|message| {
        !matches!(
            message,
            Message::System { content }
                if content.contains("<task-state>")
                    || content.contains("<task-contract>")
                    || content.contains("Project map source: docs/PROJECT_MAP.md")
                    || content.contains("<memory-context>")
        )
    }));

    drop(tx);
    let mut saw_start = false;
    while let Some(event) = rx.recv().await {
        saw_start |= matches!(event, StreamEvent::Start);
    }
    assert!(!saw_start, "quiet direct turns should not emit a run card");
}

#[tokio::test]
async fn runtime_diet_report_records_context_budget_when_compressor_enabled() {
    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::from(vec![ChatResponse {
            content: "hello".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        }])),
    });
    let trace_store = Arc::new(TraceStore::default());
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(ToolRegistry::new()),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
    .with_trace_store(trace_store.clone())
    .with_compression(8_000)
    .with_max_iterations(1);

    let result = loop_instance
        .run(vec![Message::user("请简单回复一句 hello")])
        .await
        .expect("loop should complete");

    assert_eq!(result.content, "hello");
    let trace = trace_store.latest().expect("trace should be recorded");
    let budget = trace.events.iter().find_map(|event| {
        if let TraceEvent::RuntimeDietReport {
            total_request_tokens,
            max_context_tokens,
            remaining_context_tokens,
            ..
        } = event
        {
            Some((
                *total_request_tokens,
                *max_context_tokens,
                *remaining_context_tokens,
            ))
        } else {
            None
        }
    });

    let (total, max, remaining) = budget.expect("runtime diet budget should be recorded");
    assert!(total > 0);
    assert_eq!(max, Some(8_000));
    assert!(remaining.unwrap() < 8_000);
    assert!(crate::engine::trace::format_trace_summary(&trace, 80).contains("context_remaining="));
}

#[tokio::test]
async fn runtime_diet_report_records_tool_result_budget_for_tool_turn() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS", "0");
    let tmp = tempdir().expect("create temp dir");
    let target = tmp.path().join("note.txt");
    std::fs::write(&target, "tool result budget evidence").expect("write fixture");

    let provider = Arc::new(MockLlmProvider {
        responses: StdMutex::new(VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_read".to_string(),
                    name: "file_read".to_string(),
                    arguments: serde_json::json!({
                        "path": target.to_string_lossy().to_string()
                    }),
                }]),
                usage: None,
                tool_call_repair: None,
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            },
        ])),
    });
    let mut registry = ToolRegistry::new();
    registry.register(FileReadTool);
    let trace_store = Arc::new(TraceStore::default());
    let loop_instance = ConversationLoop::new(
        provider,
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
    .with_trace_store(trace_store.clone())
    .with_max_iterations(3);

    let result = loop_instance
        .run(vec![Message::user("读取 note.txt")])
        .await
        .expect("loop should complete");

    assert_eq!(result.content, "done");
    let trace = trace_store.latest().expect("trace should be recorded");
    let tool_budget = trace.events.iter().find_map(|event| {
        if let TraceEvent::RuntimeDietReport {
            tool_result_chars,
            tool_result_tokens,
            truncated_tool_results,
            tool_result_artifacts,
            ..
        } = event
        {
            Some((
                *tool_result_chars,
                *tool_result_tokens,
                *truncated_tool_results,
                *tool_result_artifacts,
            ))
        } else {
            None
        }
    });

    let (chars, tokens, truncated, artifacts) =
        tool_budget.expect("runtime diet tool budget should be recorded");
    assert!(chars > 0);
    assert!(tokens > 0);
    assert_eq!(truncated, 0);
    assert_eq!(artifacts, 0);
    assert!(crate::engine::trace::format_trace_summary(&trace, 80).contains("tool_results="));
}

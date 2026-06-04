use super::*;
use async_trait::async_trait;
use serde_json::{json, Value};

struct IntegerParamTool;

#[async_trait]
impl Tool for IntegerParamTool {
    fn name(&self) -> &str {
        "integer_param_tool"
    }

    fn description(&self) -> &str {
        "test integer validation"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "timeout": { "type": "integer" }
            },
            "required": ["timeout"]
        })
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success("ok")
    }
}

struct ComplexSchemaTool;

#[async_trait]
impl Tool for ComplexSchemaTool {
    fn name(&self) -> &str {
        "complex_schema_tool"
    }

    fn description(&self) -> &str {
        "test structured schema validation"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["read", "write"]
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10
                },
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "minLength": 1 },
                            "kind": { "type": ["string", "null"], "enum": ["file", "dir", null] }
                        },
                        "required": ["path"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["mode", "items"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success("ok")
    }
}

#[test]
fn test_tool_result() {
    let success = ToolResult::success("Done");
    assert!(success.success);
    assert_eq!(success.content, "Done");

    let error = ToolResult::error("Failed");
    assert!(!error.success);
    assert_eq!(error.error, Some("Failed".to_string()));
}

#[test]
fn test_validate_params_accepts_integer_type_for_json_number() {
    let tool = IntegerParamTool;
    let err = tool.validate_params(&json!({ "timeout": 60 }));
    assert!(
        err.is_none(),
        "integer JSON number should pass schema validation"
    );
}

#[test]
fn test_validate_params_rejects_enum_mismatch() {
    let tool = ComplexSchemaTool;
    let err = tool
        .validate_params(&json!({
            "mode": "delete",
            "items": [{ "path": "src/lib.rs" }]
        }))
        .expect("enum mismatch should fail");

    assert!(err.contains("Parameter 'mode' must be one of"));
}

#[test]
fn test_validate_params_rejects_nested_missing_required() {
    let tool = ComplexSchemaTool;
    let err = tool
        .validate_params(&json!({
            "mode": "read",
            "items": [{ "kind": "file" }]
        }))
        .expect("nested required field should fail");

    assert_eq!(err, "Missing required parameter: items[0].path");
}

#[test]
fn test_validate_params_rejects_unknown_key_when_schema_closes_object() {
    let tool = ComplexSchemaTool;
    let err = tool
        .validate_params(&json!({
            "mode": "read",
            "items": [{ "path": "src/lib.rs", "extra": true }]
        }))
        .expect("additionalProperties=false should reject unknown key");

    assert_eq!(err, "Unknown parameter: items[0].extra");
}

#[test]
fn test_validate_params_rejects_numeric_bounds() {
    let tool = ComplexSchemaTool;
    let err = tool
        .validate_params(&json!({
            "mode": "read",
            "limit": 11,
            "items": [{ "path": "src/lib.rs" }]
        }))
        .expect("maximum should fail");

    assert!(err.contains("Parameter 'limit' must be <= 10"));
}

#[test]
fn retained_context_keeps_retrieval_and_skill_provenance() {
    let mut retrieval = crate::engine::retrieval_context::RetrievalContext::new(
        "fix tests",
        crate::engine::intent_router::RetrievalPolicy::Project,
    );
    retrieval.add_item(crate::engine::retrieval_context::RetrievalItem::new(
        crate::engine::retrieval_context::RetrievalSource::Memory,
        "Memory note",
        "Use cargo check before broad tests",
        0.9,
        "memory.prefetch",
        crate::engine::retrieval_context::TrustLevel::Medium,
    ));

    let retained =
        ToolContextRetainedContext::from_retrieval_context("fix tests", Some(&retrieval))
            .with_skill_triggers(vec![ToolContextSkillTrigger {
                name: "rust-agent".to_string(),
                description: "Repo workflow".to_string(),
                triggers: vec!["rust".to_string()],
                allowed_tools: vec!["grep".to_string()],
                disallowed_tools: Vec::new(),
                model: None,
                effort: None,
                context: Some("inherit".to_string()),
                provenance: "skills.search:/repo/skills/rust-agent".to_string(),
            }]);

    assert_eq!(retained.retrieval_items.len(), 1);
    assert_eq!(retained.skill_triggers.len(), 1);
    assert!(retained
        .provenance
        .iter()
        .any(|item| item.contains("memory.prefetch")));
    assert!(retained
        .provenance
        .iter()
        .any(|item| item == "skill_triggers=1"));
}

#[test]
fn test_tool_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(BashTool);

    assert!(registry.has("bash"));
    assert!(registry.has("shell"));
    assert_eq!(registry.get("shell").map(|tool| tool.name()), Some("bash"));
    assert!(!registry.has("nonexistent"));
}

#[test]
fn tool_schema_includes_contract_metadata() {
    let schema = FileReadTool.schema();
    assert_eq!(schema.aliases, vec!["read"]);
    assert_eq!(
        schema.search_hint.as_deref(),
        Some("view file contents directory entries")
    );
    assert!(schema.strict_schema);
    assert_eq!(schema.interrupt_behavior, ToolInterruptBehavior::Block);
    assert!(!schema.requires_user_interaction);
}

/// 一致性测试：确保所有核心工具在默认注册表中可用
/// 防止"文档写了有，模型调不到"的问题
#[test]
fn test_all_core_tools_registered() {
    let registry = ToolRegistry::with_profile(ToolRegistryProfile::Core);
    let registered = registry.tool_names();

    let expected_core = [
        "file_read",
        "file_write",
        "file_edit",
        "file_patch",
        "glob",
        "grep",
        "bash",
        "bash_output",
        "bash_cancel",
        "bash_tasks",
        "run_tests",
        "start_dev_server",
        "memory_save",
        "memory_load",
        "memory_clear",
        "todo_write",
        "git_status",
        "git_diff",
        "context",
        "context_visualization",
        "calculate",
        "datetime",
        "enter_plan_mode",
        "exit_plan_mode",
        "skill_manage",
        "skills_list",
        "skill_view",
        "ask_user",
        "plan",
    ];

    for &name in &expected_core {
        assert!(
            registered.contains(&name),
            "Core tool '{}' NOT in default_registry! Models can't call it.",
            name
        );
    }

    for &name in &[
        "agent",
        "web_fetch",
        "web_search",
        "json_query",
        "git",
        "notebook",
        "repl",
        "powershell",
        "send_message",
        "tool_search",
        "mcp",
        "mcp_tool",
        "mcp_auth",
        "list_mcp_resources",
        "read_mcp_resource",
        "lsp",
        "symbol_query",
        "worktree",
        "workbench",
        "project_list",
        "refactor",
    ] {
        assert!(
            !registered.contains(&name),
            "Extended tool '{}' should not be in the Core default surface.",
            name
        );
    }
}

#[test]
fn test_full_registry_includes_low_frequency_tools() {
    let registry = ToolRegistry::full_registry();
    let registered = registry.tool_names();

    for &name in &[
        "agent",
        "web_fetch",
        "web_search",
        "json_query",
        "encode",
        "diff",
        "format",
        "git",
        "notebook",
        "repl",
        "powershell",
        "send_message",
        "tool_search",
        "sleep",
        "socratic_analyze",
        "cron",
        "swarm",
        "mcp",
        "mcp_tool",
        "mcp_auth",
        "list_mcp_resources",
        "read_mcp_resource",
        "lsp",
        "symbol_query",
        "worktree",
        "workbench",
        "project_list",
        "refactor",
        "desktop",
        "remote_trigger",
        "remote_dev",
        "browser",
        "team",
        #[cfg(feature = "voice")]
        "voice",
        "telemetry",
        "plugin_list",
        "plugin_manage",
    ] {
        assert!(
            registered.contains(&name),
            "Full registry should include gated tool '{}'.",
            name
        );
    }
}

#[test]
fn core_tool_contract_descriptions_stay_compact() {
    let registry = ToolRegistry::with_profile(ToolRegistryProfile::Core);
    let budgets = [
        ("file_read", 320usize),
        ("file_write", 360usize),
        ("file_edit", 900usize),
        ("bash", 420usize),
        ("run_tests", 520usize),
        ("skill_view", 260usize),
    ];

    for (name, max_chars) in budgets {
        let tool = registry.get(name).expect("core tool should be registered");
        let chars = tool.description().chars().count();
        assert!(
            chars <= max_chars,
            "tool contract for '{}' is too large: {} chars > {}. Move rare guidance into failure-specific messages.",
            name,
            chars,
            max_chars
        );
    }
}

/// 工具数量不能回退
#[test]
fn test_tool_count_not_regressed() {
    let registry = ToolRegistry::full_registry();
    let count = registry.tool_names().len();
    assert!(
        count >= 50,
        "Tool count regressed! Expected >= 50, got {}",
        count
    );
}

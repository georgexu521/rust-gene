use std::sync::Arc;

use priority_agent::engine::conversation_loop::ConversationLoop;
use priority_agent::services::api::{ChatResponse, LlmProvider, Message};
use priority_agent::tools::{ToolRegistry, ToolRegistryProfile};
use tokio::sync::Mutex;

use super::mock_provider::{text_response, tool_response, MockProvider};

fn system_prompt() -> String {
    "You are a coding assistant. Use tools to read, write, and edit files as needed.".to_string()
}

fn build_loop(responses: Vec<ChatResponse>) -> ConversationLoop {
    let provider = Arc::new(MockProvider::from_responses("mock-model", responses));
    ConversationLoop::new(
        provider,
        Arc::new(ToolRegistry::with_profile(ToolRegistryProfile::Core)),
        Arc::new(Mutex::new(priority_agent::cost_tracker::CostTracker::new())),
        "mock-model".to_string(),
    )
    .with_max_iterations(3)
}

fn run_loop(
    rt: &tokio::runtime::Runtime,
    lp: ConversationLoop,
    user_msg: &str,
) -> priority_agent::engine::conversation_loop::LoopResult {
    rt.block_on(async {
        lp.run(vec![
            Message::system(system_prompt()),
            Message::user(user_msg.to_string()),
        ])
        .await
    })
    .expect("ConversationLoop::run should succeed")
}

fn project_temp_dir() -> tempfile::TempDir {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e-tests");
    std::fs::create_dir_all(&base).expect("create e2e temp base");
    tempfile::Builder::new()
        .prefix("priority-agent-e2e-")
        .tempdir_in(base)
        .expect("create isolated e2e tempdir")
}

#[test]
fn e2e_smoke_mock_provider_compiles() {
    let provider = MockProvider::new("mock-model");
    assert_eq!(provider.base_url(), "mock://e2e");
    assert_eq!(provider.default_model(), "mock-model");
}

#[test]
fn test_pure_text_flow() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let lp = build_loop(vec![text_response("Hello! I'm ready to help.")]);
    let result = run_loop(&rt, lp, "hello");
    assert_eq!(result.content, "Hello! I'm ready to help.");
    assert!(!result.tool_calls_made);
    assert!(result.tools_used.is_empty());
    assert_eq!(result.iterations, 1);
}

#[test]
fn test_file_read_flow() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = project_temp_dir();
    let test_file = dir.path().join("read_test.txt");
    std::fs::write(&test_file, "hello world\n").unwrap();

    let lp = build_loop(vec![
        tool_response(
            "file_read",
            serde_json::json!({"path": test_file.to_string_lossy()}),
        ),
        text_response("The file contains: hello world."),
    ]);

    let result = run_loop(
        &rt,
        lp,
        &format!("read {} and review the code", test_file.display()),
    );
    assert!(result.tool_calls_made);
    assert_eq!(result.tools_used, vec!["file_read"]);
    assert!(result.iterations > 1, "iterations={}", result.iterations);
}

#[test]
fn test_multi_tool_flow() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let lp = build_loop(vec![
        tool_response("file_read", serde_json::json!({"path": "Cargo.toml"})),
        tool_response(
            "grep",
            serde_json::json!({"pattern": "fn main", "path": "src"}),
        ),
        text_response("I read the config and found the main function."),
        text_response("Done."),
    ]);

    let result = run_loop(&rt, lp, "read Cargo.toml and find main function");

    assert!(result.tool_calls_made);
    assert_eq!(result.tools_used, vec!["file_read", "grep"]);
    assert!(result.iterations >= 2, "iterations={}", result.iterations);
    assert!(!result.content.is_empty());
}

#[test]
fn test_tool_failure_recovery() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let lp = build_loop(vec![
        tool_response("file_read", serde_json::json!({})),
        text_response("I recovered after the tool error."),
    ]);

    let result = run_loop(&rt, lp, "read the missing file and recover if needed");

    assert!(result.tool_calls_made);
    assert_eq!(result.tools_used, vec!["file_read"]);
    assert!(result.iterations >= 2, "iterations={}", result.iterations);
    assert!(result.content.contains("recovered"));
}

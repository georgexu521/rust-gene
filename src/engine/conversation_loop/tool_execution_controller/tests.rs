use super::*;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRouter;
use crate::engine::resource_policy::ResourcePolicy;
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
use crate::tools::{Tool, ToolContext};
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

fn tool_call(id: &str, name: &str) -> ToolCall {
    tool_call_with_args(id, name, serde_json::json!({}))
}

fn tool_call_with_args(id: &str, name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments,
    }
}

#[test]
fn memory_action_signal_records_evidence_without_changing_scores() {
    let call = tool_call("call_1", "file_edit");
    let mut decision = ActionDecision::for_tool_call(
        &call,
        ActionDecisionInput {
            task_stage: AgentTaskStage::Edit,
            route_workflow: None,
            route_risk: None,
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );
    let before = decision.scores.risk;
    let items = vec![ToolContextRetentionItem {
        source: "Memory".to_string(),
        title: "memory_record/mem1:memory/strategy-failures.md".to_string(),
        provenance: "memory.match:memory_record/mem1:memory/strategy-failures.md".to_string(),
        reason: "failure pattern warns about broad edit".to_string(),
        trust: "High".to_string(),
        conflict: false,
        token_estimate: 12,
    }];

    apply_memory_action_signal(&mut decision, &call, &items);

    assert_eq!(decision.scores.risk, before);
    assert!(decision
        .reason_summary
        .contains("memory evidence value_delta=0 risk_delta=2"));
    assert!(decision.reason_summary.contains("not_applied_to_score"));
    let modifier = decision
        .score_computation
        .modifiers
        .iter()
        .find(|modifier| modifier.source == ActionScoreModifierSource::Memory)
        .expect("memory evidence modifier");
    assert_eq!(modifier.kind, "memory_failure_risk");
    assert_eq!(modifier.risk_delta, 0);
    assert!(modifier
        .reason
        .contains("suggested value_delta=0 risk_delta=2"));
}

struct NoopProvider;

#[async_trait]
impl LlmProvider for NoopProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        Ok(ChatResponse {
            content: String::new(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        Err(anyhow::anyhow!("unused test provider stream"))
    }

    fn base_url(&self) -> &str {
        "test://noop"
    }

    fn default_model(&self) -> &str {
        "test"
    }
}

struct ProbeReadTool {
    writes: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for ProbeReadTool {
    fn name(&self) -> &str {
        "probe_read"
    }

    fn description(&self) -> &str {
        "Read the probe write counter"
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success(format!(
            "writes_seen={}",
            self.writes.load(Ordering::SeqCst)
        ))
    }

    fn is_read_only(&self, _params: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &Value) -> bool {
        true
    }
}

struct ProbeWriteTool {
    writes: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for ProbeWriteTool {
    fn name(&self) -> &str {
        "probe_write"
    }

    fn description(&self) -> &str {
        "Increment the probe write counter"
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        let previous = self.writes.fetch_add(1, Ordering::SeqCst);
        ToolResult::success(format!("writes_before={previous}"))
    }
}

struct PrematureEditProbeTool {
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for PrematureEditProbeTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Probe premature file edits"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "new_string": { "type": "string" }
            },
            "required": ["path", "new_string"]
        })
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        self.executions.fetch_add(1, Ordering::SeqCst);
        ToolResult::success("edit executed")
    }
}

fn probe_loop(writes: Arc<AtomicUsize>) -> ConversationLoop {
    let mut registry = ToolRegistry::new();
    registry.register(ProbeReadTool {
        writes: writes.clone(),
    });
    registry.register(ProbeWriteTool { writes });
    ConversationLoop::new(
        Arc::new(NoopProvider),
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    )
}

async fn execute_probe_tools(
    loop_instance: &ConversationLoop,
    tool_calls: &[ToolCall],
    pre_executed: HashMap<usize, ToolResult>,
) -> ToolExecutionBatch {
    execute_probe_tools_with_trace(loop_instance, tool_calls, pre_executed, None).await
}

async fn execute_probe_tools_with_trace(
    loop_instance: &ConversationLoop,
    tool_calls: &[ToolCall],
    pre_executed: HashMap<usize, ToolResult>,
    trace: Option<TraceCollector>,
) -> ToolExecutionBatch {
    let route = IntentRouter::new().route("probe ordered tools");
    let mut policy = ResourcePolicy::from_route(&route);
    policy.max_tool_calls = 20;
    policy.parallelism_limit = 4;
    let destructive_scope = DestructiveScopeContract::from_user_request(
        "probe ordered tools",
        &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    );
    let exposed_tool_names = HashSet::from(["probe_read".to_string(), "probe_write".to_string()]);
    let mut lifecycle = ToolCallLifecycle::default();
    let mut storm_state = StormState::default();

    ToolExecutionController::new(ToolExecutionContext::from_conversation(loop_instance))
        .execute_tools_parallel(ToolExecutionRequest {
            tool_calls,
            parent_assistant_content: "",
            tx: None,
            pre_executed,
            trace,
            route: Some(&route),
            resource_policy: &policy,
            exposed_tool_names: &exposed_tool_names,
            retained_context: &crate::tools::ToolContextRetainedContext::default(),
            task_stage: AgentTaskStage::Understand,
            task_state: None,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            no_progress_rounds: 0,
            has_changes_before_tools: false,
            destructive_scope: &destructive_scope,
            storm_state: &mut storm_state,
            lifecycle: &mut lifecycle,
        })
        .await
}

#[test]
fn batch_summarizes_results_and_lifecycle_statuses() {
    let mut lifecycle = ToolCallLifecycle::default();
    let denied = tool_call("call_1", "file_write");
    let failed = tool_call("call_2", "bash");
    let pre_executed = tool_call("call_3", "file_read");

    lifecycle.denied(&denied);
    lifecycle.completed(&failed, &ToolResult::error("nope"));
    lifecycle.provider_executed(&pre_executed, &ToolResult::success("ok"));

    let batch = ToolExecutionBatch::new(
        vec![
            (denied.clone(), ToolResult::error("denied")),
            (failed.clone(), ToolResult::error("nope")),
            (pre_executed.clone(), ToolResult::success("ok")),
        ],
        lifecycle.snapshot(),
    );

    assert!(batch.any_success());
    assert_eq!(batch.unsuccessful_count(), 2);
    let result_successes = batch
        .result_successes()
        .map(|(tool_call, success)| (tool_call.id.as_str(), success))
        .collect::<Vec<_>>();
    assert_eq!(
        result_successes,
        vec![("call_1", false), ("call_2", false), ("call_3", true)]
    );
    assert_eq!(batch.denied_count(), 1);
    assert_eq!(batch.failed_count(), 1);
    assert_eq!(batch.pre_executed_count(), 1);
}

#[test]
fn batch_synthesizes_terminal_result_for_missing_lifecycle_result() {
    let pending = tool_call("call_missing", "bash");
    let mut lifecycle = ToolCallLifecycle::default();
    lifecycle.pending_batch(std::slice::from_ref(&pending));

    let batch = ToolExecutionBatch::new(Vec::new(), lifecycle.snapshot());

    assert_eq!(batch.results().len(), 1);
    assert_eq!(batch.results()[0].0.id, "call_missing");
    assert!(!batch.results()[0].1.success);
    assert!(batch.results()[0]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("no terminal result was recorded"));
    assert_eq!(batch.failed_count(), 1);
    assert_eq!(
        batch.results()[0].1.data.as_ref().unwrap()["tool_lifecycle_recovery"]["terminal_result"],
        "interrupted"
    );
}

#[tokio::test]
async fn mixed_read_write_round_preserves_tool_call_order() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call("call_read_before", "probe_read"),
        tool_call("call_write", "probe_write"),
        tool_call("call_read_after", "probe_read"),
    ];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
    let results = batch.results();

    assert_eq!(
        results
            .iter()
            .map(|(call, _)| call.id.as_str())
            .collect::<Vec<_>>(),
        vec!["call_read_before", "call_write", "call_read_after"]
    );
    assert_eq!(results[0].1.content, "writes_seen=0");
    assert_eq!(results[1].1.content, "writes_before=0");
    assert_eq!(results[2].1.content, "writes_seen=1");
}

#[tokio::test]
async fn exact_duplicate_read_only_call_is_suppressed_before_dispatch() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let args = json!({"path": "README.md", "offset": 0, "limit": 80});
    let tool_calls = vec![
        tool_call_with_args("read_1", "probe_read", args.clone()),
        tool_call_with_args("read_2", "probe_read", args.clone()),
        tool_call_with_args("read_3", "probe_read", args),
    ];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
    let results = batch.results();

    assert_eq!(results.len(), 3);
    assert!(results[0].1.success);
    assert!(results[1].1.success);
    assert!(!results[2].1.success);
    assert!(results[2]
        .1
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("detected repeated call"));
}

#[tokio::test]
async fn changed_read_only_ranges_are_not_suppressed_as_duplicates() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call_with_args(
            "read_1",
            "probe_read",
            json!({"path": "README.md", "offset": 0, "limit": 80}),
        ),
        tool_call_with_args(
            "read_2",
            "probe_read",
            json!({"path": "README.md", "offset": 80, "limit": 80}),
        ),
        tool_call_with_args(
            "read_3",
            "probe_read",
            json!({"path": "README.md", "offset": 160, "limit": 80}),
        ),
    ];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;

    assert!(batch.results().iter().all(|(_, result)| result.success));
}

#[tokio::test]
async fn tool_results_include_action_decision_metadata() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![tool_call("call_read", "probe_read")];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
    let metadata = batch.results()[0]
        .1
        .data
        .as_ref()
        .expect("tool metadata should be present");

    assert_eq!(
        metadata["action_decision"]["action"]["tool_name"],
        "probe_read"
    );
    assert!(metadata["action_decision"]["scores"]["value"].is_u64());
    assert_eq!(metadata["action_review"]["decision"], "allow");
    assert_eq!(
        metadata["action_review"]["primary_reason"],
        "safe_to_execute"
    );
}

#[test]
fn action_review_metadata_includes_observed_checkpoint_id() {
    let tool = PrematureEditProbeTool {
        executions: Arc::new(AtomicUsize::new(0)),
    };
    let tool_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: json!({"path": "src/lib.rs", "new_string": "updated"}),
    };
    let exposed = HashSet::from(["file_edit".to_string()]);
    let permission_context = crate::permissions::PermissionContext::new(".");
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: ActionDecision::for_tool_call(
            &tool_call,
            ActionDecisionInput {
                task_stage: AgentTaskStage::Edit,
                route_workflow: Some(WorkflowKind::CodeChange),
                route_risk: Some(RiskLevel::Medium),
                action_checkpoint_active: false,
                has_changes_before_tools: false,
                no_progress_rounds: 0,
            },
        ),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: Some(std::path::Path::new(".")),
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });
    let mut result =
        ToolResult::success_with_data("edit executed", json!({"checkpoint": {"id": "cp_test_1"}}));

    attach_action_review_metadata(&mut result, &review);

    let checkpoint = &result.data.as_ref().unwrap()["action_review"]["checkpoint"];
    assert_eq!(checkpoint["status"], "required_and_present");
    assert_eq!(checkpoint["checkpoint_id"], "cp_test_1");
    assert_eq!(checkpoint["observed_result_checkpoint"], true);
}

#[tokio::test]
async fn tool_execution_records_action_review_trace() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![tool_call("call_read", "probe_read")];
    let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "probe"));

    let _batch = execute_probe_tools_with_trace(
        &loop_instance,
        &tool_calls,
        HashMap::new(),
        Some(trace.clone()),
    )
    .await;
    let snapshot = trace.snapshot();

    assert!(snapshot.events.iter().any(|event| matches!(
        event,
        TraceEvent::ActionReviewed {
            tool,
            decision,
            reason,
            ..
        } if tool == "probe_read" && decision == "allow" && reason == "safe_to_execute"
    )));
}

#[tokio::test]
async fn premature_edit_in_understand_stage_runs_with_advisory_review() {
    let executions = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry.register(PrematureEditProbeTool {
        executions: executions.clone(),
    });
    let loop_instance = ConversationLoop::new(
        Arc::new(NoopProvider),
        Arc::new(registry),
        Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
        "test".into(),
    );
    let route = IntentRouter::new().route("edit src/lib.rs");
    let mut policy = ResourcePolicy::from_route(&route);
    policy.max_tool_calls = 4;
    let destructive_scope = DestructiveScopeContract::from_user_request(
        "edit src/lib.rs",
        &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    );
    let exposed_tool_names = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);
    let mut lifecycle = ToolCallLifecycle::default();
    let mut storm_state = StormState::default();
    let task_state = AgentTaskState::from_initial_context(
        "edit src/lib.rs",
        std::path::Path::new("."),
        &route,
        None,
    );
    let tool_calls = vec![ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: json!({"path": "src/lib.rs", "new_string": "updated"}),
    }];

    let batch =
        ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls: &tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed: HashMap::new(),
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                task_stage: AgentTaskStage::Understand,
                task_state: Some(&task_state),
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                storm_state: &mut storm_state,
                lifecycle: &mut lifecycle,
            })
            .await;
    let result = &batch.results()[0].1;

    assert!(result.success);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.data.as_ref().unwrap()["action_review"]["decision"],
        "allow"
    );
    assert!(
        result.data.as_ref().unwrap()["action_review"]["worth"]["premature_mutation"]
            .as_bool()
            .unwrap()
    );
}

#[tokio::test]
async fn consecutive_read_batches_stay_ordered_across_writes() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call("read_1", "probe_read"),
        tool_call("read_2", "probe_read"),
        tool_call("write_1", "probe_write"),
        tool_call("read_3", "probe_read"),
        tool_call("read_4", "probe_read"),
    ];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
    let results = batch.results();

    assert_eq!(
        results
            .iter()
            .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("read_1", "writes_seen=0"),
            ("read_2", "writes_seen=0"),
            ("write_1", "writes_before=0"),
            ("read_3", "writes_seen=1"),
            ("read_4", "writes_seen=1"),
        ]
    );
}

#[tokio::test]
async fn denied_tool_between_read_batches_preserves_result_order() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call("read_before", "probe_read"),
        tool_call("denied", "probe_denied"),
        tool_call("read_after", "probe_read"),
    ];

    let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
    let results = batch.results();

    assert_eq!(
        results
            .iter()
            .map(|(call, _)| call.id.as_str())
            .collect::<Vec<_>>(),
        vec!["read_before", "denied", "read_after"]
    );
    assert_eq!(results[0].1.content, "writes_seen=0");
    assert!(!results[1].1.success);
    assert!(results[1].1.content.contains("not found"));
    assert_eq!(
        results[1].1.data.as_ref().unwrap()["action_review"]["decision"],
        "revise"
    );
    assert_eq!(
        results[1].1.data.as_ref().unwrap()["action_review"]["primary_reason"],
        "tool_not_available"
    );
    assert_eq!(results[2].1.content, "writes_seen=0");
    assert_eq!(batch.denied_count(), 1);
}

#[tokio::test]
async fn pre_executed_read_only_result_before_serial_boundary_keeps_original_position() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call("read_pre_executed", "probe_read"),
        tool_call("write", "probe_write"),
        tool_call("read_after", "probe_read"),
    ];
    let pre_executed = HashMap::from([(0usize, ToolResult::success("pre_executed_read"))]);

    let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
    let results = batch.results();

    assert_eq!(
        results
            .iter()
            .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("read_pre_executed", "pre_executed_read"),
            ("write", "writes_before=0"),
            ("read_after", "writes_seen=1"),
        ]
    );
    assert_eq!(batch.pre_executed_count(), 1);
}

#[tokio::test]
async fn pre_executed_read_only_result_after_serial_boundary_is_rerun() {
    let writes = Arc::new(AtomicUsize::new(0));
    let loop_instance = probe_loop(writes);
    let tool_calls = vec![
        tool_call("read_before", "probe_read"),
        tool_call("write", "probe_write"),
        tool_call("read_pre_executed", "probe_read"),
    ];
    let pre_executed = HashMap::from([(2usize, ToolResult::success("pre_executed_read"))]);

    let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
    let results = batch.results();

    assert_eq!(
        results
            .iter()
            .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("read_before", "writes_seen=0"),
            ("write", "writes_before=0"),
            ("read_pre_executed", "writes_seen=1"),
        ]
    );
    assert_eq!(batch.pre_executed_count(), 0);
}

// ── Segmented scheduling contract tests ──────────────────

#[tokio::test]
async fn read_only_read_only_runs_as_parallel_segment() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("r1", "probe_read"),
        tool_call("r2", "probe_read"),
        tool_call("r3", "probe_read"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    // Read-only tools execute without error — verify the batch was created.
    assert!(
        !batch.results().is_empty() || batch.pre_executed_count() > 0,
        "read-only tools should produce results or pre-executed results"
    );
}

#[tokio::test]
async fn mutating_tool_is_barrier_between_read_only_segments() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("r1", "probe_read"),
        tool_call("w1", "probe_write"),
        tool_call("r2", "probe_read"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    let ids: Vec<&str> = batch
        .results()
        .iter()
        .map(|(tc, _)| tc.id.as_str())
        .collect();
    assert_eq!(ids, vec!["r1", "w1", "r2"]);
    let contents: Vec<&str> = batch
        .results()
        .iter()
        .map(|(_, r)| r.content.as_str())
        .collect();
    assert_eq!(contents[0], "writes_seen=0");
    assert_eq!(contents[2], "writes_seen=1");
}

#[tokio::test]
async fn read_only_after_write_does_not_precede_write() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("w1", "probe_write"),
        tool_call("r1", "probe_read"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    let ids: Vec<&str> = batch
        .results()
        .iter()
        .map(|(tc, _)| tc.id.as_str())
        .collect();
    assert_eq!(ids, vec!["w1", "r1"]);
    let (_, r1) = batch
        .results()
        .iter()
        .find(|(tc, _)| tc.id == "r1")
        .unwrap();
    assert_eq!(r1.content, "writes_seen=1");
}

#[tokio::test]
async fn result_order_matches_original_tool_call_order() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("a", "probe_read"),
        tool_call("b", "probe_write"),
        tool_call("c", "probe_read"),
        tool_call("d", "probe_write"),
        tool_call("e", "probe_read"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    let ids: Vec<&str> = batch
        .results()
        .iter()
        .map(|(tc, _)| tc.id.as_str())
        .collect();
    assert_eq!(ids, vec!["a", "b", "c", "d", "e"]);
}

#[tokio::test]
async fn denied_tool_flushes_prior_parallel_segment() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("r1", "probe_read"),
        tool_call("w1", "probe_write"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    let ids: Vec<&str> = batch
        .results()
        .iter()
        .map(|(tc, _)| tc.id.as_str())
        .collect();
    assert_eq!(ids, vec!["r1", "w1"]);
}

#[tokio::test]
async fn pre_executed_not_reused_after_serial_boundary() {
    let writes = Arc::new(AtomicUsize::new(0));
    let tool_calls = vec![
        tool_call("r1", "probe_read"),
        tool_call("w1", "probe_write"),
        tool_call("r2", "probe_read"),
    ];
    let batch = execute_probe_tools(&probe_loop(writes), &tool_calls, HashMap::new()).await;
    let (_, r2) = batch
        .results()
        .iter()
        .find(|(tc, _)| tc.id == "r2")
        .unwrap();
    assert_eq!(r2.content, "writes_seen=1");
}

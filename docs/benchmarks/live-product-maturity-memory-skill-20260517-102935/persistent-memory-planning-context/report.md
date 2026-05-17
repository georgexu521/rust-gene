# Live Eval Report: persistent-memory-planning-context

- Run id: `product-maturity-memory-skill-20260517-102935`
- Sample: `evalsets/live_tasks/persistent-memory-planning-context.yaml`
- Worktree: `target/live-evals/product-maturity-memory-skill-20260517-102935/persistent-memory-planning-context/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-20260517-102935/persistent-memory-planning-context/env`
- Test status: `failed`
- Generated: `2026-05-17 11:07:43 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q learning_planning -- --test-threads=1
warning: fields `memory_manager`, `provider`, and `model` are never read
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:19:16
   |
14 | pub(super) struct TurnRetrievalContextRequest<'a> {
   |                   --------------------------- fields in this struct
...
19 |     pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
   |                ^^^^^^^^^^^^^^
20 |     pub(super) provider: &'a dyn LlmProvider,
   |                ^^^^^^^^
21 |     pub(super) model: &'a str,
   |                ^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: associated functions `build_memory_context` and `record_memory_prefetch` are never used
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:57:14
   |
27 | impl TurnRetrievalContextController {
   | ----------------------------------- associated functions in this implementation
...
57 |     async fn build_memory_context(
   |              ^^^^^^^^^^^^^^^^^^^^
...
84 |     fn record_memory_prefetch(trace: &TraceCollector, context: &RetrievalContext) {
   |        ^^^^^^^^^^^^^^^^^^^^^^


running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1432 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q retrieval_context -- --test-threads=1
warning: fields `memory_manager`, `provider`, and `model` are never read
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:19:16
   |
14 | pub(super) struct TurnRetrievalContextRequest<'a> {
   |                   --------------------------- fields in this struct
...
19 |     pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
   |                ^^^^^^^^^^^^^^
20 |     pub(super) provider: &'a dyn LlmProvider,
   |                ^^^^^^^^
21 |     pub(super) model: &'a str,
   |                ^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: associated functions `build_memory_context` and `record_memory_prefetch` are never used
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:57:14
   |
27 | impl TurnRetrievalContextController {
   | ----------------------------------- associated functions in this implementation
...
57 |     async fn build_memory_context(
   |              ^^^^^^^^^^^^^^^^^^^^
...
84 |     fn record_memory_prefetch(trace: &TraceCollector, context: &RetrievalContext) {
   |        ^^^^^^^^^^^^^^^^^^^^^^


running 16 tests
................
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 1421 filtered out; finished in 0.01s

[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/turn_retrieval_context_controller.rs'; s=open(p).read(); assert 'prefetch_retrieval_context_with_llm_rerank' in s and 'Self::merge_context(&mut turn_retrieval_context, memory_ctx)' in s and 'TraceEvent::MemoryPrefetch' in s"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/engine/conversation_loop/workflow_contract_controller.rs'; s=open(p).read(); assert 'apply_learning_to_workflow_judgment' in s and 'context.retrieval_context' in s"
[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/mod.rs'; s=open(p).read(); ctx=s.find('TurnContextBootstrapController::run'); gate=s.find('TurnEntryGateController::run'); assert ctx >= 0 and gate >= 0 and ctx < gate"
[exit status: 0]

$ cargo test -q -- --test-threads=1
warning: fields `memory_manager`, `provider`, and `model` are never read
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:19:16
   |
14 | pub(super) struct TurnRetrievalContextRequest<'a> {
   |                   --------------------------- fields in this struct
...
19 |     pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
   |                ^^^^^^^^^^^^^^
20 |     pub(super) provider: &'a dyn LlmProvider,
   |                ^^^^^^^^
21 |     pub(super) model: &'a str,
   |                ^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: associated functions `build_memory_context` and `record_memory_prefetch` are never used
  --> src/engine/conversation_loop/turn_retrieval_context_controller.rs:57:14
   |
27 | impl TurnRetrievalContextController {
   | ----------------------------------- associated functions in this implementation
...
57 |     async fn build_memory_context(
   |              ^^^^^^^^^^^^^^^^^^^^
...
84 |     fn record_memory_prefetch(trace: &TraceCollector, context: &RetrievalContext) {
   |        ^^^^^^^^^^^^^^^^^^^^^^


running 1437 tests
....................................................................................... 87/1437
....................................................................................... 174/1437
....................................................................................... 261/1437
....................................................................................... 348/1437
....................................................................................... 435/1437
....................................................................................... 522/1437
....................................................................................... 609/1437
....................................................................................... 696/1437
....................................................................................... 783/1437
....................................................................................... 870/1437
....................................................................................... 957/1437
....................................................................................... 1044/1437
....................................................................................... 1131/1437
....................................................................................... 1218/1437
....................................................................................... 1305/1437
....................................................................................... 1392/1437
.............................................
test result: ok. 1437 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.54s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/persistent-memory-planning-context/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/persistent-memory-planning-context/agent-events.jsonl`

Event counts:

```text
eval_started: 1
permission_request: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
diff_files_changed: 0
tool_executions: 0
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 13
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: missing
adaptive_triggers: required_validation
trace_event_types: intent,resource.policy,retrieval.context,workflow.trigger,workflow.fallback,workflow.fallback,task.context,implementation.intent,reflection.pass,permission.request,permission.resolve,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_planning_context,memory_retrieval_before_workflow_judgment
behavior_assertion_status: failed
warning: empty_agent_output
warning: no_code_diff
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 2/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 6
agent_required_commands: 6
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: missing
runtime_diet: missing
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO

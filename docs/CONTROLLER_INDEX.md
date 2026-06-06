# Controller Responsibility Index

Each controller in `src/engine/conversation_loop/` has a single owner and
test file. This index maps controller → responsibility → caller → tests.

## Request Preparation

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `request_preparation_controller` | Build ChatRequest: dynamic zones, memory prefetch, output cap, message healing | `turn_model_step_controller` |
| `preflight_compression_controller` | Pre-turn context compaction | `turn_setup_controller` |
| `context_budget_controller` | Token budget observation + runtime diet recording | `request_preparation_controller` |

## Turn Lifecycle

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `turn_setup_controller` | Per-turn initialization: route, tools, validation, state | `conversation_loop` |
| `turn_model_step_controller` | LLM request → response → tool extraction | `turn_loop_state_controller` |
| `turn_assistant_response_controller` | Process LLM response: text, tool calls, closeout | `turn_model_step_controller` |
| `turn_completion_controller` | Final closeout, repo checks, execution report | `turn_loop_state_controller` |
| `turn_iteration_controller` | Single iteration: request → tools → response | `conversation_loop` |
| `turn_iteration_loop_controller` | Multi-iteration loop: repair, retry, closeout | `conversation_loop` |
| `turn_iteration_setup_controller` | Per-iteration setup: tool batch, route context | `turn_iteration_loop_controller` |
| `turn_iteration_closeout_controller` | Iteration closeout: gate checks, iteration stop | `turn_iteration_loop_controller` |

## Tool Execution

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `tool_execution_controller` | Tool dispatch, timeout, background shell | `tool_turn_controller` |
| `tool_result_controller` | Structured observation evidence from tool results | `tool_turn_controller` |
| `tool_batch_result_processor` | Batch result processing: corrections, route recovery, risk gates | `tool_turn_controller` |
| `tool_turn_controller` | Tool turn orchestration: batch execution → results | `turn_iteration_controller` |
| `tool_round_controller` | Per-round tool execution within a batch | `tool_turn_controller` |
| `tool_failure_stop_controller` | Stop loops on severe tool failures | `tool_turn_controller` |

## Closeout

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `closeout_controller` | Verification proof, evidence, acceptance, execution report | `turn_completion_controller` |
| `turn_post_change_closeout_controller` | Post-change validation + closeout flow | `turn_completion_controller` |
| `post_change_workflow_controller` | Workflow steps after code change: test, validate | `turn_post_change_closeout_controller` |
| `first_code_change_controller` | First code change detection for closeout gating | `closeout_controller` |
| `legacy_workflow_gate_controller` | Legacy verification gates (deprecating) | `closeout_controller` |

## Repair

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `focused_repair_state_controller` | Repair state tracking: consecutive failures, checkpoint | `tool_batch_result_processor` |
| `turn_focused_repair_action_controller` | Repair action selection: prompt, re-read, revert | `turn_iteration_controller` |
| `turn_focused_repair_flow_controller` | Full repair flow: detect failure → apply repair | `turn_completion_controller` |
| `repair_controller` | Generic repair: regeneration, context escalation | `turn_focused_repair_flow_controller` |
| `post_edit_repair_controller` | Edit failure recovery: stale anchor, line mismatch | `tool_batch_result_processor` |
| `post_edit_verification_controller` | Post-edit verify: LSP diagnostics, diff audit | `post_edit_repair_controller` |
| `patch_synthesis_flow_controller` | Patch synthesis for failed edits | `focused_repair_state_controller` |

## Permissions

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `permission_controller` | Permission check, deny/allow/ask, tool family | `tool_execution_controller` |
| `risk_signal_controller` | Risk signal detection: injection, traversal, secrets | `permission_controller` |

## API

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `api_request_controller` | Provider API call: timeout, retry, slow-tail | `turn_model_step_controller` |
| `assistant_response_retry_controller` | Retry on response failure | `turn_assistant_response_controller` |
| `turn_api_failure_controller` | API failure handling: fallback, circuit | `turn_completion_controller` |

## Context

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `turn_context_bootstrap_controller` | Per-turn context assembly: memory, skills, retrieval | `turn_setup_controller` |
| `turn_retrieval_context_controller` | Retrieval context: memory search, project index | `turn_context_bootstrap_controller` |
| `retrieval_prompt_controller` | Retrieval prompt injection | `request_preparation_controller` |
| `memory_snapshot_controller` | Memory snapshot freeze + prefetch | `turn_context_bootstrap_controller` |
| `memory_sync_controller` | Memory sync after tool writes | `tool_batch_result_processor` |

## Misc

| Controller | Responsibility | Called By |
|------------|---------------|-----------|
| `session_goal_controller` | Session goal tracking | `turn_setup_controller` |
| `iteration_budget_controller` | Iteration budget enforcement | `turn_iteration_loop_controller` |
| `reflection_gate_controller` | Reflection gate: should agent self-reflect? | `turn_completion_controller` |
| `task_context_trace_controller` | Task context → trace event | `turn_task_context_controller` |
| `workflow_contract_controller` | Workflow contract: required validation, scope | `turn_setup_controller` |
| `turn_entry_gate_controller` | Entry gate: should this turn continue? | `turn_loop_state_controller` |
| `turn_loop_bootstrap_controller` | Loop bootstrap: state, tools, context | `turn_loop_state_controller` |
| `turn_loop_state_controller` | Loop state machine: entry → iterate → closeout | `conversation_loop` |
| `turn_request_bootstrap_controller` | Request bootstrap: messages, tools, temp | `turn_model_step_controller` |
| `turn_runtime_diet_bootstrap_controller` | Runtime diet: token tracking, tool exposure | `turn_setup_controller` |
| `turn_task_context_controller` | Task context: state, observations, hypotheses | `turn_setup_controller` |
| `turn_tool_failure_followup_controller` | Failure followup: what to try next | `turn_completion_controller` |
| `turn_tool_round_outcome_controller` | Round outcome: success/failure/repair decision | `turn_tool_round_step_controller` |
| `turn_tool_round_step_controller` | Round step: execute tools, process results | `turn_iteration_controller` |

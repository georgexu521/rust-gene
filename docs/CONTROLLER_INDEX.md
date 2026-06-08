# Controller Responsibility Index

This index maps the active `src/engine/conversation_loop/` runtime modules to
their current responsibility and primary caller. It intentionally tracks the
post-merge module names; archived docs may still mention older controllers.

## Request Preparation

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `request_preparation_controller` | Build ChatRequest: dynamic zones, memory prefetch, output cap, message healing | `turn_model_step_controller` |
| `turn_request_bootstrap_controller` | Request messages, route metadata, retrieval prompt injection, tool schema prep | `turn_model_step_controller` |
| `preflight_compression_controller` | Pre-turn context compaction | `turn_setup_controller` |
| `context_budget_controller` | Token budget observation and runtime diet recording | `request_preparation_controller` |
| `request_timeouts` | Shared API and streaming timeout policy | `api_request_controller`, `session_processor` |

## Turn Lifecycle

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `turn_setup_controller` | Per-turn initialization: route, tools, validation, bootstrap state | `conversation_loop` |
| `turn_entry_gate_controller` | Entry gating, session goal update, task context trace emission | `conversation_loop` |
| `turn_loop_bootstrap_controller` | Loop bootstrap, runtime diet bootstrap, context/tool readiness | `conversation_loop` |
| `turn_state` | Turn loop state, runtime state, session runtime context | `turn_loop_bootstrap_controller`, iteration controllers |
| `turn_model_step_controller` | LLM request, response retry, tool extraction | `turn_iteration_controller` |
| `turn_assistant_response_controller` | Assistant text/tool-call processing and response lifecycle | `turn_model_step_controller` |
| `turn_iteration_setup_controller` | Per-iteration setup and route-scoped tool exposure plan | `turn_iteration_loop_controller` |
| `turn_iteration_controller` | Single iteration: request, tool turn, post-change closeout bridge | `turn_iteration_loop_controller` |
| `turn_iteration_closeout_controller` | Iteration closeout checks and stop reasons | `turn_iteration_loop_controller` |
| `turn_iteration_loop_controller` | Multi-iteration loop, repair retry, final closeout handoff | `conversation_loop` |
| `turn_completion_controller` | Final closeout, repo checks, execution report | `turn_iteration_loop_controller` |
| `turn_loop_policy` | Main-loop profile, force-summary prompt, fallback summary policy | `conversation_loop` |

## Tool Execution

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `tool_execution_controller` | Tool dispatch, timeout, background shell, permission bridge | `tool_turn_controller` |
| `tool_execution` | Low-level execution helpers, tool output truncation, concurrency safety | `tool_execution_controller` |
| `tool_result_controller` | Structured observation evidence from tool results | `tool_turn_controller` |
| `tool_batch_result_processor` | Batch result processing: corrections, route recovery, risk gates | `tool_turn_controller` |
| `tool_turn_controller` | Tool turn orchestration: batch execution to result processing | `turn_iteration_controller` |
| `tool_round_controller` | Per-round batch execution and iteration budget enforcement | `tool_turn_controller` |
| `turn_tool_round_step_controller` | Convert tool-round results into turn step outcomes | `turn_iteration_controller` |
| `tool_failure_guided_debugging` | Tool failure follow-up hints and guided debugging prompts | `turn_completion_controller`, tool result flow |

## Closeout

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `closeout_controller` | Verification proof, evidence, acceptance, final execution report | `turn_completion_controller` |
| `post_change_workflow_controller` | Post-change validation workflow and first-change recording | `turn_iteration_controller` |
| `first_code_change_controller` | First code change detection for closeout gating | `post_change_workflow_controller` |
| `legacy_workflow_gate_controller` | Legacy verification gates while newer proof gates remain active | `closeout_controller` |
| `validation_runner` | Shell validation execution and verification source helpers | `closeout_controller`, tests |

## Repair

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `focused_repair_state_controller` | Repair state tracking: consecutive failures, checkpoint context | `tool_batch_result_processor` |
| `turn_focused_repair_flow_controller` | Full repair flow: detect failure, apply repair, re-enter loop | `turn_completion_controller` |
| `repair_controller` | Generic repair, regeneration, context escalation | `turn_focused_repair_flow_controller` |
| `post_edit_repair_controller` | Edit failure recovery: stale anchors and line mismatch | `tool_batch_result_processor` |
| `post_edit_verification_controller` | Post-edit verify: diagnostics, diff audit | `post_edit_repair_controller` |
| `patch_synthesis_flow_controller` | Test-only patch synthesis flow coverage | test modules |

## Permissions And Risk

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `permission_controller` | Permission check, deny/allow/ask, tool family policy | `tool_execution_controller` |
| `permission_recovery` | Permission recovery hints kept out of the large permission controller | `permission_controller` |
| `risk_signal_controller` | Risk signal detection: prompt injection, traversal, secrets | `permission_controller` |
| `approval` | Approval request/response channel types | `permission_controller`, UI/runtime callers |
| `action_checkpoint` | Checkpoint coordination for mutating actions | `tool_execution_controller` |

## API

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `api_request_controller` | Provider API call: timeout, retry, slow-tail handling | `turn_model_step_controller` |
| `assistant_response_retry_controller` | Retry on malformed or failed assistant response | `turn_assistant_response_controller` |
| `turn_api_failure_controller` | API failure handling: fallback, circuit, final turn status | `turn_completion_controller` |
| `session_processor` | Streaming/non-streaming session processing bridge | `conversation_loop` |

## Context And Memory

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `turn_context_bootstrap_controller` | Per-turn context assembly: memory, skills, retrieval | `turn_setup_controller` |
| `turn_retrieval_context_controller` | Retrieval context collection from project/session/memory sources | `turn_context_bootstrap_controller` |
| `memory_snapshot_controller` | Memory snapshot freeze and prefetch | `turn_context_bootstrap_controller` |
| `memory_sync_controller` | Memory sync after tool writes | `tool_batch_result_processor` |
| `runtime_diet` | Runtime diet snapshot and token/tool exposure observations | bootstrap and request prep |
| `companion_context` | Companion-side context snippets for final/runtime behavior | context bootstrap |

## Workflow And Task State

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `workflow_contract_controller` | Workflow contract: required validation, scope, proof expectations | `turn_setup_controller` |
| `workflow_prompt_policy` | Prompt policy for workflow guidance | request/context prep |
| `workflow_runtime` | Runtime workflow activation and learning events | turn lifecycle |
| `workflow_trace` | Workflow trace event helpers | workflow/runtime callers |
| `workflow_change_tracker` | Changed-file and workflow-change tracking | closeout/tool result flow |
| `turn_task_context_controller` | Task context state, observations, hypotheses | `turn_setup_controller` |
| `task_guidance_controller` | Route/task guidance construction | `turn_setup_controller` |
| `reflection_gate_controller` | Reflection gate and self-reflection eligibility | `turn_completion_controller` |

## Utility Modules

| Module | Responsibility | Called By |
|--------|----------------|-----------|
| `tool_metadata` | Tool execution metadata and lifecycle rendering | tool execution/result flow |
| `tool_call_lifecycle` | Tool-call lifecycle events | tool execution/result flow |
| `tool_context_helpers` | Tool context and denial helper functions | tool execution flow |
| `tool_orchestrator` | Tool orchestration helpers | tool turn flow |
| `pseudo_tool_text` | Pseudo-tool text parsing/rendering | assistant response flow |
| `text_sanitizer` | Visible text sanitization | response/final rendering |
| `turn_recording` | Turn recording helpers | turn lifecycle |
| `step_executor` | Real workflow step execution bridge | workflow/runtime flow |

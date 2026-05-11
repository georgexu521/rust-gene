# Conversation Loop Responsibility Map

Date: 2026-05-11

This note freezes the current `ConversationLoop::run_inner` responsibility map
before the next extraction pass. The goal is to make each follow-up change say
which responsibility is moving and which module owns it after the move.

## Current Ownership

| Responsibility | Current Location | Target Owner | Migration State |
|---|---|---|---|
| prompt/context assembly | `conversation_loop/mod.rs`, `prompt_context`, retrieval helpers | `PromptContextBuilder` plus `ContextBudgetController` | mixed |
| intent route and resource policy | `conversation_loop/mod.rs`, `intent_router`, `resource_policy` | `TurnRuntimeState` setup plus route policy modules | mixed |
| model request and streaming | `session_processor.rs`, `conversation_loop/mod.rs` retry loop | `SessionProcessor` | partial |
| tool exposure | `conversation_loop/mod.rs`, `tools/mod.rs`, route filters | `ToolExposureController` / `PermissionController` | partial |
| tool call lifecycle | `conversation_loop/mod.rs`, `tool_execution_controller.rs`, `session_processor.rs` | `SessionProcessor` plus `ToolCallLifecycle` | mixed |
| tool result normalization | `tool_metadata.rs`, `tool_result_controller.rs`, scattered truncation calls | `ToolResultNormalizer` | partial |
| evidence ledger | `conversation_loop/mod.rs`, `evidence_ledger` | `TurnRuntimeState` owns ledger, controllers append facts | started |
| repair/action checkpoint | `conversation_loop/mod.rs`, `repair_controller.rs`, `action_checkpoint.rs`, `patch_recovery.rs` | `RepairController` plus explicit checkpoint state | mixed |
| validation | `conversation_loop/mod.rs`, `validation_runner.rs` | `ValidationRunner` | partial |
| closeout | `closeout_controller.rs`, `conversation_loop/mod.rs` | `CloseoutController` | partial |
| memory/learning persistence | `conversation_loop/mod.rs`, `turn_recording.rs`, memory manager | `TurnRecording` / memory services | partial |
| trace/session store | `conversation_loop/mod.rs`, `session_processor.rs`, `turn_recording.rs` | `TraceController` / `TurnRecording` | partial |

## First Boundary Decision

`TurnRuntimeState` now owns the mutable state that belongs to one assistant
turn and is shared by multiple controllers:

- `EvidenceLedger`
- `RuntimeDietSnapshot`
- iteration and repair counters

This is intentionally small. It moves data ownership first without changing
runtime behavior. Later batches can add route, resource policy, exposed tools,
pending tool calls, and checkpoint state after the current behavior has a clean
test baseline.

`SessionStepResult` now names the output of one model request / streaming step:

- assistant-visible text
- collected tool calls
- pre-executed read-only tool results
- usage
- finish reason
- step source

This replaces the old anonymous tuple and gives the future `SessionProcessor`
state machine an explicit return boundary.

`ToolResultNormalizer` now owns the first provider-facing normalization hook.
The initial implementation preserves the existing model content exactly, but
future tool result changes should extend this boundary instead of adding raw
stdout/stderr parsing or provider-specific formatting back into `run_inner`.

`ToolCallLifecycle` now records the state of tool calls during execution:
pending, running, completed, failed, denied, and provider-executed. The current
implementation is attached to `TurnRuntimeState` and updates alongside the
existing tool execution path without changing returned tool results or UI
events.

`ToolExecutionBatch` now names the result of one tool execution batch. It still
preserves the old ordered tool result list, but also carries lifecycle-derived
denied, failed, and pre-executed counts so the future session state machine can
route on structured facts instead of rescanning raw tuples. The main loop now
uses batch-level success and unsuccessful-result summaries for low-risk guard
and retry decisions while leaving detailed evidence collection unchanged.

`ToolExecutionRequest` now names the input context for one tool execution
batch. It keeps the existing execution semantics, but removes the long
anonymous argument list and makes route policy, exposed tools, checkpoint facts,
destructive scope, pre-executed results, streaming, and lifecycle ownership
explicit at the controller boundary.

`ToolExecutionController` now owns the `execute_tools_parallel` implementation.
The first ownership split borrowed `ConversationLoop` for existing
dependencies; the follow-up split replaced that broad borrow with an explicit
execution context.

`ToolExecutionContext` now snapshots the concrete dependencies needed by
`ToolExecutionController`: registry, cost tracker, session id/store, hooks,
approval channel, allowed tools, audit/denial trackers, active goal, and base
tool context. The controller no longer holds a broad `ConversationLoop` borrow
while executing tools.

`ToolExecutionGate` now owns pre-execution allow/deny decisions for a tool call:
exposed-tool enforcement, resource budget, goal-drift trace, allowed-tools
isolation, destructive scope, and action checkpoint bash/file_edit guards. The
controller remains responsible for persistence, lifecycle updates, scheduling,
and result ordering.

## Extraction Rule

Future loop-split commits should follow this rule:

1. Move state ownership into `TurnRuntimeState` before moving logic.
2. Move one responsibility at a time.
3. Keep provider-facing message shape and tool-result order unchanged unless a
   provider protocol test changes with it.
4. Run the narrow gate for the moved responsibility before broadening to
   `cargo check -q`.

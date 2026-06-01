# Runtime Diet Update

Date: 2026-06-02

Status: implemented in commit `4430647b`.

## Decision

The runtime loop should stay simple and deterministic. The model owns semantic
judgment and decides when it has enough information. The runtime owns hard
limits, tool execution, permission and path safety, evidence capture,
validation proof, and honest closeout gating.

This update deliberately removes pseudo-intelligent runtime behavior that tried
to infer model intent from repeated reads, no-progress scores, low action
scores, or workflow-specific finish heuristics.

## Current Loop Contract

- `StreamingQueryEngine` remains the canonical full runtime for CLI, TUI,
  headless dogfood, and desktop.
- A valid assistant response with no valid tool calls finishes the turn. The
  runtime no longer gives programming workflows an extra non-tool retry just
  because the response looked like "thinking aloud".
- Empty assistant content is still treated as provider/model failure and gets a
  bounded retry prompt when iteration budget remains.
- Iteration budget remains the primary tool-loop safety net. The default cap is
  50, with force-summary injected near the end of the budget.
- Exact repeated tool storms are handled by the shared storm guard. `file_read`
  is no longer exempt, so identical repeated reads are bounded there. Different
  paths, offsets, or limits remain different calls.

## Removed Runtime Intervention

The following behavior was removed:

- duplicate successful read-only pre-execution closeout;
- cached read-only result substitution before executing a mixed tool batch;
- duplicate directory-read redirection;
- ledger-based runtime-generated final answers for repeated reads;
- model-visible "stop reading and answer from prior output" system prompts;
- stop-check decisions based only on low action scores, no-progress rounds,
  uncertainty-not-reduced counters, repeated failed tools, repeated validation
  failures, or duplicate read-only counts.

These signals may still be useful as trace or task-state evidence, but they no
longer control the loop as a shadow planner.

## Kept Hard Constraints

- user interruption;
- high-risk/user permission decisions;
- action review deny/ask/revise outcomes;
- rollback candidate checkpointing;
- max-iteration budget exhaustion;
- bounded invalid model-output repair;
- verification-ready closeout based on successful validation evidence;
- destructive-scope and file/path permission gates;
- provider-visible tool-result truncation, with raw artifacts kept out of the
  model-facing text by default.

## Files Changed

- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/conversation_loop/tool_batch_result_processor.rs`
- `src/engine/conversation_loop/tool_execution.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/conversation_loop/tool_call_lifecycle.rs`
- `src/engine/conversation_loop/turn_runtime_state.rs`
- `src/engine/conversation_loop/turn_tool_round_outcome_controller.rs`
- `src/engine/stop_checker.rs`
- `src/tools/file_tool/mod.rs`
- `scripts/tui-dogfood-test.sh`

## Validation

Run on 2026-06-02:

```bash
cargo fmt --check
cargo check -q
cargo test -q --lib stop_checker
cargo test -q --lib turn_iteration_controller
cargo test -q --lib tool_batch_result_processor
cargo test -q --lib tool_execution_controller
cargo test -q --lib resolve_read_path_rejects_runtime_tool_result_artifacts_by_default
bash -n scripts/tui-dogfood-test.sh
git diff --check
```

No desktop, TUI, or live dogfood run was performed for this update.

## Follow-Up

The next validation step should be a headless/TUI complex-runtime dogfood run,
then a small desktop smoke only if the runtime behavior is stable. Do not add
new workflow branches to fix one-off model mistakes unless they can be stated
as a hard tool contract, permission rule, evidence rule, or validation rule.

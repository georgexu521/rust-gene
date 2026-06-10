# Parallel Tool Scheduling + LLM Compaction Plan

Date: 2026-06-10
Status: Active
Parents: `docs/AGENT_SKILLS_OPTIMIZATION_PLAN_2026-06-01.md`, `docs/LLM_COMPACTION_PLAN_2026-06-08.md`

## Part 1: True Parallel Tool Scheduling

### Goal

Change tool execution from sequential batching to two-phase parallel scheduling:
scan all tool calls first, run all read-only tools concurrently, then run
mutating tools sequentially. This is the last remaining optimization from the
AGENT_SKILLS_OPTIMIZATION plan (5/6 done).

### Current State

`src/engine/conversation_loop/tool_execution_controller.rs:608`
(`execute_tools_parallel`) already has the building blocks:
- `parallel_jobs` buffer for accumulating concurrent work
- `serial_boundary_seen` flag that flushes on the first non-concurrent tool
- `collect_read_only_results()` for awaiting parallel results
- `tool_call_is_concurrency_safe()`, `tool_call_is_read_only()`,
  `force_serial_tool_dispatch()`, `read_only_tool_concurrency()` for dispatch

But the current loop processes tools in arrival order: it accumulates read-only
tools until it hits a mutating tool, flushes the batch, runs the mutating tool,
then repeats. This is **sequential batching**, not true two-phase scheduling.

### opcode Reference

opencode (TypeScript) dispatches tool calls in two clear phases:
1. Identify all independent (read-only) tools → spawn them in parallel
2. Run dependent (mutating) tools sequentially

This is visible in the task tool description itself: "Launch multiple agents
concurrently whenever possible, to maximize performance; use a single message
with multiple tool uses."

The same principle applies to tool scheduling: read-only ops (grep, glob,
file_read) should never block each other.

### Gap

The current code has all the infrastructure but the loop control flow prevents
true two-phase execution. The fix is local to `execute_tools_parallel`:

**Current (sequential batching):**
```
for each tool_call:
    if concurrency-safe → add to parallel buffer
    else → flush buffer, run serial, continue
flush remaining buffer
```

**Target (two-phase):**
```
phase 1: scan all tool_calls, categorize into read_only[] and mutating[]
phase 2: run all read_only tools in parallel (respecting concurrency limit)
phase 3: run all mutating tools sequentially
```

### Implementation

File: `src/engine/conversation_loop/tool_execution_controller.rs`

- Add a `scan_tool_calls()` helper that iterates once and returns
  `(Vec<ReadOnlyJob>, Vec<MutatingJob>)` without executing anything.
- In `execute_tools_parallel`, call scan first, then run all read-only
  jobs through `collect_read_only_results()`, then run mutating jobs
  sequentially.
- Preserve: storm detection, gate evaluation, pre-executed results,
  serial boundary semantics for trace events.

The change is ~40 lines of refactoring within a single function. No API changes,
no new modules, no config flags.

### Verification

```bash
cargo test -q tool_execution_controller --lib
cargo test -q tool_execution --lib
cargo test -q tool_batch_result_processor --lib
```

---

## Part 2: LLM Compaction — Phase 2 Prompt Hardening

### Goal

Strengthen the LLM summary prompt contract so that compacted context preserves
structured evidence and the closeout controller can distinguish between raw
validation proof and compressed summaries.

### Current State

From `docs/LLM_COMPACTION_PLAN_2026-06-08.md` Phase 2 (not done):

The code already has:
- `ContextCompressor::llm_summarize_middle()` — calls LLM for summary
- `SUMMARY_TEMPLATE` and `SUMMARY_PREFIX` — existing prompt templates
- `CompactionAttemptRecord`, `CompactionRuntimeRecord` — metadata

But the prompt sent to the LLM does **not** use the 8-section strict contract.
It's a loose prompt asking for a summary. The result is unpredictable — the LLM
may or may not preserve tool names, exit codes, file paths, and diff status.

### What opcode Does

opencode's context compaction (from the research in AGENTS.md and project
discussions) uses structured summary contracts. The compaction agent produces
output in a rigid format that downstream code can parse:
- What was attempted
- What commands were run
- What files were changed
- What the results were
- What remains to be done

This structured output allows the runtime to distinguish between "this evidence
came from a real tool execution" and "this was summarized by an LLM."

### Gap

Priority Agent's `LLM_COMPACTION_PLAN` Phase 2 lists the required strict prompt
contract with 8 sections. This prompt exists in the plan document but was never
integrated into `ContextCompressor::llm_summarize_middle()`.

The closeout controller also trusts compacted history equally with raw history,
which violates the evidence-aware compaction principle.

### Implementation

#### Step A: Add strict summary prompt

File: `src/engine/context_compressor/compressor.rs`

Replace or extend the current summary prompt with the structured contract:

```text
You are summarizing a section of a coding session. Preserve only what matters
for future turns. Output a compact summary in this exact format:

## Goal
(one line: what the user asked for)

## Changes
- file: line-age description (omit if no changes)

## Commands
- command → exit_code (omit if none)

## Validation
- pass | fail | not_run

## Errors
- description (omit if none)

## Status
- done | in_progress | blocked

## Next
(one line: what the model should do next)

## Evidence
- source: key fact (tool name, diff status, command output summary)
```

#### Step B: Gate closeout on evidence source

File: `src/engine/conversation_loop/closeout_controller/mod.rs`

When evaluating closeout evidence, check whether it comes from a compacted
boundary. If evidence is from a compacted summary (not raw tool execution),
label it as `compacted_summary` rather than `verified`. The closeout can still
accept it for narrative context but must not treat it as verification proof.

#### Step C: Wire env variable

File: `src/engine/context_compressor/compressor.rs`

Respect `PRIORITY_AGENT_LLM_COMPACTION=1` from the plan. Currently the gate
might use a different env name. Ensure it's consistently:
- `0` / unset → LLM compaction off (default)
- `1` → use the new strict prompt contract

### Verification

```bash
cargo test -q context_compressor --lib
cargo test -q closeout_controller --lib
cargo test -q streaming --lib
```

---

## Implementation Order

```
Part 1 (parallel scheduling) → Part 2 Step A (prompt) → Part 2 Step B (closeout gate) → Part 2 Step C (env flag)
```

Part 1 and Part 2 are independent. Part 1 should go first since it's a
performance optimization with no behavioral change.

## Total Files Changed

| File | Part | Change |
|------|------|--------|
| `tool_execution_controller.rs` | 1 | Two-phase scan-refactor (~40 lines) |
| `context_compressor/compressor.rs` | 2A | New strict prompt template (~30 lines) |
| `closeout_controller/mod.rs` | 2B | Evidence source check (~10 lines) |
| `context_compressor/compressor.rs` | 2C | Env gate consistency (~5 lines) |

## Validation

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings

# Part 1
cargo test -q tool_execution_controller --lib
cargo test -q tool_execution --lib
cargo test -q tool_batch_result_processor --lib

# Part 2
cargo test -q context_compressor --lib
cargo test -q closeout_controller --lib

# Full
cargo test -q
bash scripts/daily-baseline.sh
```

## Non-Goals

- No new compaction strategy or system — reuse existing ContextCompressor
- No automatic compaction threshold change (keep 80%, not 60%)
- No tool API changes — no new Tool trait methods
- No config flag changes for parallel scheduling (reuse existing concurrency limit)
- No LLM compaction enabled by default (keep PRIORITY_AGENT_LLM_COMPACTION=0)

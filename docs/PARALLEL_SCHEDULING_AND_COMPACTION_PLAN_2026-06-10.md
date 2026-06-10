# Parallel Scheduling And Compaction Evidence Plan

Date: 2026-06-10
Status: Active
Parents: `docs/AGENT_SKILLS_OPTIMIZATION_PLAN_2026-06-01.md`,
`docs/LLM_COMPACTION_PLAN_2026-06-08.md`

## Goal

Improve two runtime surfaces without weakening ordering, permissions, or proof:

1. Make tool scheduling easier to reason about and test while preserving
   model-requested order across mutating boundaries.
2. Harden compaction evidence boundaries so LLM summaries remain continuation
   context and never become runtime verification proof.

The original draft was too aggressive in both areas. Current code already has
substantial parallel tool infrastructure and LLM compaction prompt hardening.
The next step is targeted hardening, not a broad scheduling or compaction
rewrite.

## Findings From Code Audit

### Finding 1: Global two-phase scheduling is unsafe

`src/engine/conversation_loop/tool_execution_controller.rs` currently executes
read-only tools in parallel windows and treats mutating or non-concurrency-safe
tools as serial barriers.

That behavior is conservative for a reason. A tool call sequence like this must
not be globally reordered:

```text
file_write("src/lib.rs")
file_read("src/lib.rs")
```

The `file_read` is read-only, but it depends on the preceding write. A global
"scan all read-only first, then run all mutating tools" scheduler would read the
old file content and feed stale evidence back to the model.

Safe target:

```text
segment 1: consecutive independent read-only tools -> run concurrently
barrier: mutating / permissioned / non-concurrency-safe tool -> run serially
segment 2: next consecutive independent read-only tools -> run concurrently
```

This is closer to the current implementation than the original plan suggested.
The implementation work should make this segmented contract explicit and better
tested, not move every read-only call ahead of every mutating call.

### Finding 2: Existing scheduling already has important side effects

`execute_tools_parallel` is not a simple dispatcher. It also handles:

- storm suppression before dispatch;
- `ToolExecutionGate` review and resource policy checks;
- action checkpoint restrictions;
- pre-executed streaming results;
- permission requests for mutating tools;
- tool lifecycle state;
- learning-event persistence;
- trace and UI start/complete events;
- result ordering for provider-visible tool results.

A scheduling refactor must preserve all of these surfaces. It should not be
described as a 40-line local rewrite unless tests prove the behavior stays
equivalent.

### Finding 3: LLM compaction prompt hardening is already partly implemented

The original draft said the strict 8-section prompt was not integrated. Current
code already has:

- `SUMMARY_TEMPLATE` and `SUMMARY_PREFIX` in `src/engine/context_compressor.rs`;
- `llm_summarize_middle()` gated by `PRIORITY_AGENT_LLM_COMPACTION`;
- strict sections for Goal, Constraints, Progress, Key Decisions, Relevant
  Files, Next Steps, Critical Context, and Tools & Patterns;
- prompt rules saying summary is continuation context, not verification proof;
- tests that set `PRIORITY_AGENT_LLM_COMPACTION=1`;
- runtime continuity extraction tests that preserve changed files, validation,
  terminal tasks, permissions, diagnostics, and agent state.

The remaining work is not "add the prompt from scratch." It is to tighten tests
and provenance so evidence safety is enforced across compaction boundaries.

### Finding 4: Closeout should stay ledger-backed

`closeout_controller` already evaluates closeout through `EvidenceLedger` and
`VerificationProof`. That is the right boundary. The closeout layer should not
parse compacted summary text or compact-boundary markers directly to decide
verification.

Safer target:

- raw tool execution and validation results can enter `EvidenceLedger`;
- compacted LLM summaries can be shown as narrative continuation context;
- verification proof must be derived from ledger facts, not summary text.

If a compacted summary mentions "Validation passed", that can guide the next
turn, but it must not become `VerificationProofStatus::Verified` unless raw
ledger evidence still exists.

## Implementation Plan

### Step 1: Document and test segmented tool scheduling

File:

- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_execution_controller/tests.rs`

Make the current intended contract explicit:

- read-only calls in the same dependency segment may run concurrently;
- mutating, permissioned, denied, storm-suppressed, or non-concurrency-safe calls
  are barriers;
- read-only calls after a barrier must not start before the barrier completes;
- provider-visible result order stays aligned to the original tool-call order;
- pre-executed streaming results are only reused before a serial boundary, as
  current code already does with `serial_boundary_seen`.

This can be implemented as comments plus tests first. Only extract a helper if
the tests show the current loop is too hard to reason about.

Suggested helper, if useful:

```rust
enum ScheduledToolSegment {
    ReadOnly(Vec<usize>),
    Serial(usize),
}
```

The helper should describe scheduling order only. It should not run gates,
permissions, hooks, or tools.

Verification:

```bash
cargo test -q tool_execution_controller --lib
```

Required tests:

- `read_only_read_only_runs_as_parallel_segment`
- `mutating_tool_is_barrier_between_read_only_segments`
- `read_only_after_file_write_does_not_precede_write`
- `denied_tool_flushes_prior_parallel_segment`
- `pre_executed_results_not_reused_after_serial_boundary`
- `result_order_matches_original_tool_call_order`

### Step 2: Add scheduling trace metadata without changing semantics

Files:

- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_execution_controller/runtime_context.rs`

If the tests pass and the contract is clear, add lightweight metadata to tool
runtime output:

- `segment_index`
- `segment_kind = read_only_parallel | serial`
- `barrier_reason = mutating | permission | denied | storm | non_concurrency_safe`

This gives performance/debug visibility without changing dispatch behavior.

Verification:

```bash
cargo test -q tool_execution_controller --lib
cargo test -q conversation_loop -- --test-threads=1
```

### Step 3: Tighten LLM compaction prompt tests

Files:

- `src/engine/context_compressor/compressor.rs`
- `src/engine/context_compressor/tests.rs`

The prompt already exists. Add regression coverage that captures the actual LLM
summary request and asserts it includes:

- all 8 required sections;
- "summary is continuation context, NOT verification proof";
- exact-path and exact-command preservation rules;
- "Do not claim tests passed unless raw test output evidence remains";
- previous-summary anchored update instructions when `previous_summary` exists;
- no dynamic `<context_zones>` system message copied into the stable prefix.

Do not change the env flag name. The current gate is
`PRIORITY_AGENT_LLM_COMPACTION`, and tests already use it.

Verification:

```bash
cargo test -q context_compressor --lib
```

### Step 4: Enforce summary-not-proof through ledger tests

Files:

- `src/engine/evidence_ledger.rs`
- `src/engine/conversation_loop/closeout_controller/tests.rs`

Add a regression test that builds a compacted-summary-like message containing a
claim such as `Validation passed: cargo test -q`, then evaluates closeout with
an empty `EvidenceLedger`.

Expected result:

- verification proof is `not_run` or `not_applicable`, depending on task type;
- closeout is not promoted to verified solely from summary text;
- no `required validation passed` acceptance line appears.

If current code already passes this because closeout only reads `EvidenceLedger`,
keep the test as a guard. Avoid adding compact-boundary parsing to closeout.

Verification:

```bash
cargo test -q closeout_controller --lib
```

### Step 5: Optional evidence provenance expansion

Only if Step 4 reveals ambiguity, extend `EvidenceLedger` records with a small
source/provenance marker:

```text
source_kind = raw_tool_result | runtime_validation | parent_verified | compacted_summary
```

Then teach verification proof to treat `compacted_summary` as context-only
support, never direct proof.

This is optional because the current architecture already keeps compacted
summary text outside `EvidenceLedger`.

## Implementation Order

1. Add segmented scheduling tests.
2. Add scheduling metadata only if useful.
3. Add LLM compaction prompt regression tests.
4. Add summary-not-proof closeout regression test.
5. Consider evidence provenance only if tests expose a real gap.

Part 1 and Part 2 are independent, but both touch runtime proof and trace
surfaces. Keep each change small and test-gated.

## Files Likely Touched

| File | Change |
|------|--------|
| `src/engine/conversation_loop/tool_execution_controller.rs` | scheduling comments/helper/metadata |
| `src/engine/conversation_loop/tool_execution_controller/tests.rs` | segmented scheduling tests |
| `src/engine/conversation_loop/tool_execution_controller/runtime_context.rs` | optional segment metadata |
| `src/engine/context_compressor/compressor.rs` | prompt wording only if tests reveal a gap |
| `src/engine/context_compressor/tests.rs` | LLM prompt contract tests |
| `src/engine/conversation_loop/closeout_controller/tests.rs` | summary-not-proof regression |
| `src/engine/evidence_ledger.rs` | optional provenance only if needed |

## Validation

Narrow gates:

```bash
cargo test -q tool_execution_controller --lib
cargo test -q context_compressor --lib
cargo test -q closeout_controller --lib
cargo test -q conversation_loop -- --test-threads=1
```

Broader gate after code changes:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
bash scripts/daily-baseline.sh
```

## Non-Goals

- No global reordering of all read-only tools ahead of mutating tools.
- No new compaction strategy or parallel `llm_compaction.rs` state machine.
- No automatic compaction threshold change.
- No default enablement of LLM compaction.
- No tool trait or provider API changes.
- No closeout proof derived from compacted summary text.

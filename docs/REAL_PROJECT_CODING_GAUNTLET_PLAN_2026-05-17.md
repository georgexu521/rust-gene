# Real Project Coding Gauntlet Plan

Date: 2026-05-17

## Why This Phase Exists

The runtime simplification and core-coding-quality phases have closed their
current scope. The next product question is no longer whether Priority Agent has
basic file, shell, validation, memory, and repair primitives. It does. The next
question is whether those primitives produce reliable coding outcomes on real
project work.

This phase turns that question into a repeatable loop:

1. run realistic coding tasks in isolated worktrees;
2. collect diff, validation, repair, and closeout evidence;
3. compare clean passes, repaired passes, and failures over time;
4. use failures to prioritize generic repair planning and durable tool records.

Non-goals for this phase:

- voice, visual polish, or decorative CLI work;
- another broad `run_inner` decomposition pass;
- hidden prompt overfitting for one live-eval fixture;
- claiming Claude/opencode parity from narrow deterministic tests.

## Current Baseline

Use `docs/PROJECT_STATUS.md` as the canonical status source. As of this plan,
the relevant baseline is:

- deterministic local tests: `1444 passed; 0 failed`;
- `core-quality-real-rerun-20260517-091952`: `8/8 passed`;
- `product-maturity-seeded-fixes-20260517-143047`: `3/3 passed`;
- active live-eval runner: `scripts/run_live_eval.sh`;
- shared report parser: `scripts/live_eval_report_parser.py`.

These are good foundations, but the task set is still too small and too close to
the code paths we have been tuning. The gauntlet must make the next failures
observable rather than hiding them behind a single pass/fail number.

## Phase 1: Gauntlet And Report Format

Status: complete for the first measurable slice.

Deliverables:

- Add a named live-eval suite: `real-project-coding`.
- Reuse `evalsets/live_tasks/*.yaml` instead of creating a parallel harness.
- Extend run summaries with coding-specific evidence:
  - agent-run coding task count;
  - real code-change pass count;
  - likely clean pass count;
  - repaired pass count;
  - failed coding task count;
  - required-validation pass count;
  - first-write availability;
  - tool executions, validation events, repair signals, changed-file count.
- Keep the existing task matrix stable so old summary and aggregate tooling keep
  working.

Initial suite composition should reuse the strongest existing real coding tasks:

- `backend-todo-api-crud`
- `frontend-book-notes-localstorage`
- `code-change-verification-repair-loop`
- `core-simple-stale-edit`
- `core-multi-file-edit`
- `core-terminal-install-run`
- `core-long-output-artifact`
- `core-provider-roundtrip`
- `core-permission-rejection-recovery`
- `core-rollback-product-path`
- `live-eval-dashboard-summary`
- `memory-save-quality-gate`
- `skill-promotion-gate`
- `persistent-memory-planning-context`

This is intentionally not the final 20-30 task suite. It is the first stable
surface for measuring the loop before adding more repositories.

Validation for Phase 1:

```bash
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
scripts/run_live_eval.sh --list --case real-project-coding
```

Validated checkpoint:

```text
real-project-coding-20260517-153331: 13/15 passed
failed tasks:
- memory-save-quality-gate: failure_owner=llm_reasoning
- persistent-memory-planning-context: failure_owner=agent_flow
```

Targeted closure after the first gauntlet:

```text
memory-save-rerun-20260517-170500: status=ok, failure_owner=none
persistent-memory-rerun-20260517-172000: status=ok, failure_owner=none
```

The failed persistent-memory rerun
`persistent-memory-rerun-20260517-171000` is kept as repair evidence: it
converted the original missing-context failure into a concrete
`build_memory_context(context)` borrow error, which is now covered by a
deterministic repair rule.

## Phase 2: Real Task Expansion

Goal: grow the gauntlet to 20-30 tasks across several project shapes.

Task sources:

- historical Priority Agent bugs and feature fixes;
- small Python/JS/Rust fixtures with real tests and edge cases;
- CLI workflow defects;
- memory/skill/product-maturity failures;
- provider/protocol and terminal/file-quality regressions.

Each task needs:

- explicit `eval_intent`;
- required commands that prove behavior;
- diff constraints;
- forbidden paths;
- human-review prompts;
- a failure owner when it fails.

Target mix:

| Type | Count | Purpose |
|------|------:|---------|
| Bug fixes | 8-10 | Reproduce, localize, repair, validate. |
| Medium features | 6-8 | Cross-file implementation and tests. |
| Refactors | 3-4 | Preserve behavior with minimal diff. |
| CLI/terminal flows | 3-4 | Long-running, output, and recovery behavior. |
| Memory/skill flows | 3-4 | Product maturity and context discipline. |

## Phase 3: Generic Repair Planner

Goal: move from fixture-specific deterministic patch rules to generic repair
planning.

The repair planner should accept structured evidence and produce bounded patch
candidates:

- compiler/test/LSP error;
- file path and line context;
- current source snippet;
- prior changed files;
- failed command;
- available validation command.

Initial repair classes:

- Rust compiler arity/type/borrow errors with exact source context;
- missing imports and stale symbol names;
- failed unit tests with assertion message and nearby implementation;
- no-diff seeded code-change failures after enough inspection;
- stale read/edit failures that require reread before patching.

Hard boundary: the repair planner proposes candidates; normal tool permission,
file-state, validation, and closeout gates still decide whether they are safe
and successful.

## Phase 4: ToolExecutionRecord Persistence

Goal: make tool execution evidence a durable runtime object instead of a set of
partially overlapping traces and rendered strings.

The record should cover:

- tool call id, name, arguments hash, and route;
- permission decision and source;
- start/end time, success, error code;
- structured output metadata;
- diff/file evidence for write tools;
- command category, validation family, and terminal task id for shell tools;
- repair/closeout relevance flags.

Consumers:

- closeout evidence;
- live-eval report parser;
- repair planner;
- session replay/debugging;
- future user-visible trace views.

## Operating Rules

- Prefer live evidence over intuition.
- Keep docs aligned with actual validated baselines.
- Do not add runtime constraints just because one live task failed.
- Treat `agent_flow`, `llm_reasoning`, `tooling`, `eval_harness`, and
  `environment` separately.
- Every new automatic repair path needs a focused deterministic test and a live
  rerun target.

## Next Concrete Step

Rerun the full gauntlet from the repaired commit, then move into the generic
repair and durable tool-record phases:

```bash
scripts/run_live_eval.sh --case real-project-coding --mode agent-run --run-tests --label real-project-coding
scripts/run_live_eval.sh --mode summary --run-id <run-id>
```

After the next full gauntlet run, choose the highest-frequency remaining
failure class:

- no diff after enough inspection -> improve action/repair planner;
- validation failure after diff -> improve generic repair planner;
- success but bad closeout -> improve ToolExecutionRecord/closeout evidence;
- environment/provider instability -> keep product code unchanged and harden
  harness/provider health reporting.

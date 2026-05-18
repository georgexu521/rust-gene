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

- deterministic local tests after memory, repair-planner, and the first
  ToolExecutionRecord persistence/trace-visibility slices:
  `1454 passed; 0 failed`;
- `core-quality-real-rerun-20260517-091952`: `8/8 passed`;
- `product-maturity-seeded-fixes-20260517-143047`: `3/3 passed`;
- `real-project-coding-20260517-192347`: `15/15 passed`;
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
- `core-inspection-grounding`
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

Post-repair full rerun:

```text
real-project-coding-20260517-171819: 12/15 passed
behavior_assertions_passed=3/3
memory-save-quality-gate: status=ok, failure_owner=none
persistent-memory-planning-context: status=ok, failure_owner=none
skill-promotion-gate: status=ok, failure_owner=none
remaining failures:
- backend-todo-api-crud: validation failed after diff; GET /todos/{id} returned 404
- frontend-book-notes-localstorage: validation failed after diff; generated JS had an extra brace
- core-provider-roundtrip: required command passed, but closeout stayed not_verified
```

Phase 3/closeout evidence rerun:

```text
real-project-coding-20260517-183221: 14/15 passed
behavior_assertions_passed=3/3
required-validation passes=15/15
backend-todo-api-crud: status=ok, failure_owner=none
frontend-book-notes-localstorage: status=ok, failure_owner=none
core-provider-roundtrip: status=ok, failure_owner=none
remaining failure:
- core-terminal-install-run: required command passed, but closeout stayed
  not_verified because an exploratory pre-install import check remained in
  runtime validation evidence
targeted closure:
- terminal-closeout-20260517-191432: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed,
  runtime validation=passed:2/2
```

Full gauntlet refresh after closeout evidence:

```text
real-project-coding-20260517-192347: 15/15 passed
behavior_assertions_passed=3/3
required-validation passes=15/15
coding-gauntlet likely clean passes=7, repaired passes=4
real code-change passes=10, audit/no-diff passes=5
failure_owner=none for every case
closeout_status=passed for every case
memory-save-quality-gate: status=ok, behavior_assertions=passed
persistent-memory-planning-context: status=ok, behavior_assertions=passed
skill-promotion-gate: status=ok, behavior_assertions=passed
core-terminal-install-run: status=ok, failure_owner=none
```

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

Status: in progress. The first generic slice is complete: failed required
validation output is parsed for source locations, current source snippets are
fed back into post-edit repair context, and the targeted backend/frontend
validation-failed-after-diff reruns now pass.

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
- failed unit tests with assertion message and nearby implementation
  (first required-validation source-context slice complete);
- no-diff seeded code-change failures after enough inspection;
- stale read/edit failures that require reread before patching.

Hard boundary: the repair planner proposes candidates; normal tool permission,
file-state, validation, and closeout gates still decide whether they are safe
and successful.

## Phase 4: ToolExecutionRecord Persistence

Goal: make tool execution evidence a durable runtime object instead of a set of
partially overlapping traces and rendered strings.

Status: in progress. A small prerequisite is in place: final closeout can prefer
exact required-command evidence over broader exploratory validation facts, so
no-diff audit tasks can close out when their required commands passed even if
earlier environment probes failed as expected. The durable record spine is also
in place: `EvidenceLedger` now stores a structured `ToolExecutionRecord` for
each tool result, final closeout trace events expose the current tool record
count for report/debug consumers, and records retain the route/resource-policy
context that shaped tool execution.

Current record coverage:

- tool call id, tool name, status, arguments hash, duration, output size, error
  code, and error preview;
- permission decision when a permission request is present;
- shell command, command category, validation family, and closeout-safety
  classification;
- terminal task id/status/kind/handle/output path/duration/exit code when
  available;
- changed paths for successful file write/edit/patch tools;
- validation, closeout, and repair relevance flags;
- route intent/workflow/retrieval/reasoning/risk context;
- resource-policy latency, parallelism, tool-call, context-budget, fallback, and
  cost ceilings;
- execution-mode flags for parallel, pre-executed, action-checkpoint, prior
  change, and exposed-tool-count state;
- permission request id/session id, approval outcome, matched patterns,
  always-allow provenance, permission family, decision, risk, and approval-source
  flags.

The record should cover:

- start/end timestamps;
- richer structured output metadata;
- diff/file evidence links for write tools;
- route-level repair/closeout relevance policy.

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

The first Phase 3 validation-failed-after-diff slice has targeted evidence:

```text
repair-planner-frontend-20260517-181652: status=ok, failure_owner=none
repair-planner-backend-20260517-182004: status=ok, failure_owner=none
backend warnings=earlier_verification_failed_before_repair,
earlier_stage_validation_failed_before_repair
```

That refresh is now done:

```text
real-project-coding-20260517-192347: 15/15 passed
behavior_assertions_passed=3/3
required-validation passes=15/15
failure_owner=none for every case
```

Next, continue into the broader Phase 4 ToolExecutionRecord persistence work.
The immediate 15-task product loop is green; Phase 4 should make the same tool
execution and closeout evidence durable instead of relying on overlapping trace
and rendered-string paths.

Use future failures to classify the next repair slice:

- no diff after enough inspection -> improve action/repair planner;
- validation failure after diff -> improve generic repair planner;
- success but bad closeout -> improve ToolExecutionRecord/closeout evidence;
- environment/provider instability -> keep product code unchanged and harden
  harness/provider health reporting.

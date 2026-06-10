# Coding Flow Polish And Observability Plan
Status: Active

Date: 2026-05-28

Goal: improve the traditional coding-agent loop without rewriting the main
conversation loop. The focus is polish, observability, and clean LLM/runtime
boundaries.

## Scope

This pass intentionally stays small:

- project-scope active self-evolution guidance so one repo's learned behavior
  does not silently affect another repo;
- trace-visible long-running validation heartbeat so users and diagnostics can
  tell that required validation is still running;
- small query API cleanup where exposed options are not honored;
- targeted tests and validation.

Out of scope:

- main loop rewrite;
- new workflow stack;
- broad prompt-rule additions;
- social/idle heartbeat behavior;
- automatic live-eval import for improvement effects.

## Baseline Judgment

The current programming-agent flow is already structurally sound:

- intent route decides broad task class and risk;
- route- and phase-scoped tools reduce model-visible surface;
- tool execution gates enforce exposure, permissions, destructive scope, and
  action review;
- tool results become structured observations and evidence ledger facts;
- validation proof controls closeout honesty;
- repair loops feed concrete failures back to the model;
- memory and self-evolution now use proposal/review/apply boundaries.

The remaining risk is not "missing agent loop". It is boundary polish:

- active learned guidance is still too global;
- long-running validation has console heartbeat but not unified trace/status;
- a few public knobs do not cleanly feed into runtime requests;
- code-change tool exposure can still be narrowed later by phase.

## First Batch

### 1. Project-scoped active guidance

Problem: active guidance currently loads from the default global guidance store
and filters mostly by message keywords.

Plan:

- add optional project identity metadata to `AppliedGuidanceRecord`;
- stamp applied guidance with the current canonical working directory;
- filter runtime guidance injection by current project identity;
- keep legacy records with missing project metadata as conservative global
  records only when they are explicitly global or diagnostic-safe.

Definition of done:

- guidance applied in one project is not injected for another project;
- current-project guidance still injects when the turn matches;
- old records deserialize safely;
- tests cover same-project, different-project, and legacy records.

### 2. Trace-visible required validation heartbeat

Problem: required validation can run without a hard timeout, but the heartbeat is
currently mostly terminal output.

Plan:

- emit a `required_validation.heartbeat` trace event every heartbeat tick;
- include command preview, elapsed seconds, and timeout mode;
- surface the latest heartbeat in runtime diagnostics.

Definition of done:

- long-running required validation records trace heartbeat events;
- unlimited timeout mode is visible;
- runtime diagnostic can show the latest running validation command;
- tests cover heartbeat event formatting and diagnostic extraction.

### 3. Query option cleanup

Problem: `QueryOptions.temperature` exists, but the main tool loop currently
uses a fixed request temperature.

Plan:

- thread optional temperature through `ConversationLoop`;
- keep default behavior at `0.2`;
- let callers override it through `QueryOptions`.

Definition of done:

- default query behavior is unchanged;
- non-default `QueryOptions.temperature` reaches the prepared `ChatRequest`;
- tests cover default and overridden temperature.

## Follow-up Batch Candidates

- phase-level tool exposure refinement for CodeChange/BugFix;
- `ContextZoneRegistry` with stable/dynamic zone budgets and fingerprints;
- automatic improvement effect import from live-eval summaries;
- full-suite test isolation pass for intermittent `run_tests_tool` failures.

## Implementation Result

Implemented in branch `codex/coding-flow-polish-observability`.

Code changes:

- applied self-evolution guidance now records optional project identity and is
  injected only for matching project scope, except explicitly global runtime
  guidance;
- required validation heartbeat now records `required_validation.heartbeat`
  trace events with command preview, elapsed seconds, and timeout mode;
- desktop runtime diagnostics now expose the latest required-validation
  heartbeat;
- `QueryOptions.temperature` now reaches the conversation loop request instead
  of being shadowed by a fixed `0.2` value.

Local validation:

- `cargo fmt --check`
- `cargo check -q`
- `cargo test -q improvement`
- `cargo test -q validation_runner`
- `cargo test -q request_preparation_controller`
- `cargo test -q turn_completion_controller`
- `cargo test -q query_engine`
- `cargo test -q memory -- --test-threads=1`
- `cargo clippy --all-features -- -D warnings`
- `bash scripts/coding-workflow-gates.sh quick`

Real-project gauntlet:

```bash
bash scripts/run_live_eval.sh \
  --case real-project-coding \
  --mode agent-run \
  --run-tests \
  --run-id coding-polish-real-20260528-123600 \
  --label coding-polish-real \
  --overlay-working-tree
bash scripts/run_live_eval.sh --mode summary --run-id coding-polish-real-20260528-123600
```

Result: `12/17` passed, `5/17` failed. Summary:
`docs/benchmarks/live-coding-polish-real-20260528-123600/summary.md`.

Interpretation:

- all five failures are classified as `failure_owner=llm_reasoning`;
- proof support was `verified` for the twelve passing tasks and `failed` for
  the five failing tasks;
- risky tool runs were all reviewed (`44/44`) and no risky tool was missing
  action-review evidence;
- runtime-spine assertions passed (`4/4`) and all tasks had trace summaries;
- required validation heartbeat was visible in long-running cases such as
  `code-change-verification-repair-loop`,
  `persistent-memory-planning-context`, and
  `memory-stale-project-fact-demotion`.

The gauntlet did not show a framework regression in this batch. The remaining
failures are model completion failures: the runtime blocked verified closeout
when required commands or behavior assertions were not satisfied. The next
useful follow-up is not another main-loop rewrite; it is a smaller repair loop
around behavior-assertion tasks where the model makes no effective code diff.

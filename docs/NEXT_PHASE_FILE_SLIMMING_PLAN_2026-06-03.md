# Next Phase File Slimming Plan

Date: 2026-06-03

Status: proposed next-phase engineering plan

## Summary

The first file-slimming pass removed the worst risk: no non-generated
engineering file is currently over 3000 lines.

The next phase should not chase a mechanical 1000-line rule. The goal is to
keep the project easy to change by reducing high-frequency runtime, tool, TUI,
desktop, and validation files into clearer responsibility boundaries.

Use 1000 lines as a pressure signal, not a hard blocker. A 1200-line focused
test fixture is less risky than a 2500-line runtime file that mixes state,
policy, rendering, and side effects.

## Current Snapshot

Latest local report:

```bash
scripts/file-size-report.sh --threshold 3000
# files: 0

scripts/file-size-report.sh --threshold 1500
# files: 45

scripts/file-size-report.sh --threshold 1000
# files: 73
```

Rough over-1000 breakdown:

```text
runtime/tooling source: 60
Rust test files:        8
scripts:                2
desktop frontend:       3
```

This means the repo is no longer in a severe oversized-file state, but it still
has enough 1500-3000 line files that future runtime and UI work can become slow
or risky if new behavior keeps landing in the same large modules.

## Engineering Standard

Use this as the next-phase standard:

- hard gate: no ordinary non-generated source file over 3000 lines;
- strong warning: runtime, tool, TUI, desktop, or script files over 2000 lines
  need an active split plan;
- soft target: high-frequency files should trend toward 800-1500 lines;
- acceptable exception: test-only files can exceed 1000 lines when fixture
  locality is valuable and the file has a narrow behavior scope;
- new modules should stay below 1000 lines unless the exception is documented.

This should be enforced by review discipline first. CI should start with a
3000-line hard gate and a non-blocking report for 1500/1000-line files.

## Non-Goals

- Do not split files by arbitrary line ranges.
- Do not weaken runtime validation, permissions, checkpoints, closeout proof,
  or memory review gates while slimming.
- Do not mix behavior rewrites with mechanical extraction.
- Do not add abstraction layers that make call paths harder to trace.
- Do not keep splitting test fixtures if that makes expected behavior harder to
  inspect.

## Priority Bands

### Band 1: High-Frequency Runtime And Tool Boundaries

These files are still large and likely to receive future behavior changes.
They should be split before adding significant new runtime features.

| File | Lines | Reason | Recommended Boundary |
|------|-------|--------|----------------------|
| `src/memory/provider.rs` | 2910 | memory is a product differentiator and provider behavior will keep changing | provider traits, local provider implementation, static-prefix assembly, diagnostics |
| `src/tools/file_tool/mod.rs` | 2820 | core safety boundary; still mixes contracts, execution, and formatting | read/list/search, write/edit, result rendering, contracts |
| `src/tools/agent_tool/mod.rs` | 2520 | subagent/worktree path is complex and high risk | request parsing, execution, worktree lifecycle, result formatting |
| `src/tools/memory_tool/mod.rs` | 2357 | memory user surface should stay easy to reason about | save/search/list/review command handlers, renderers, schemas |
| `src/tools/bash_tool/mod.rs` | 2226 | bash execution and background task handling are safety-sensitive | execution, background task API, output rendering, policy handoff |
| `src/tools/mod.rs` | 2197 | registry/orchestration file is broad | registry assembly, route exposure, schema collection |
| `src/permissions/mod.rs` | 2001 | permission policy is a hard trust boundary | rule model, matching, persistence, review summaries |

Acceptance for Band 1:

```bash
cargo fmt --check
cargo check -q
cargo test -q file_tool -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q memory -- --test-threads=1
cargo test -q permission -- --test-threads=1
```

### Band 2: Runtime Repair Loop And Engine Control Files

These files are not over 3000 lines, but they are in the core runtime path.
Split only by real phase boundaries.

| File | Lines | Reason | Recommended Boundary |
|------|-------|--------|----------------------|
| `src/engine/conversation_loop/patch_recovery.rs` | 2725 | complex repair loop, high product value | patch diagnostics, recovery plan selection, synthesis handoff |
| `src/engine/conversation_loop/tool_execution_controller.rs` | 2682 | tool execution control is critical | preflight, dispatch, observation conversion, repair feedback |
| `src/engine/conversation_loop/request_preparation_controller.rs` | 2147 | prompt/context preparation changes often | context assembly, tool exposure, budget accounting |
| `src/engine/conversation_loop/tool_result_controller.rs` | 2125 | tool output handling and synthesis triggers are easy to regress | result normalization, duplicate suppression, repair signal extraction |
| `src/engine/streaming.rs` | 2181 | shared provider stream path | stream parsing, tool-call assembly, usage accounting |
| `src/engine/mcp.rs` | 2668 | MCP behavior has auth/repair/tooling complexity | resource read, repair hints, panel/status rendering |
| `src/engine/task_context.rs` | 2424 | task context affects prompt and memory behavior | context item model, renderers, token accounting |
| `src/engine/workflow_contract.rs` | 2310 | deterministic workflow gates are policy-heavy | contract model, policy checks, report rendering |
| `src/engine/evidence_ledger.rs` | 2306 | proof boundary must stay legible | record extraction, validation classification, markdown summaries |

Acceptance for Band 2:

```bash
cargo fmt --check
cargo check -q
cargo test -q conversation_loop -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo test -q runtime_spine_behavior_contract_covers_context_action_progress_stop_and_proof -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
```

### Band 3: TUI And Desktop Real-Path Maintainability

These files are user-facing and easy to grow accidentally. Prioritize command
and state boundaries over visual changes.

| File | Lines | Reason | Recommended Boundary |
|------|-------|--------|----------------------|
| `src/tui/app.rs` | 2992 | still close to the hard gate and receives many state changes | state model, input handling, session/runtime integration |
| `src/tui/slash_handler/agents.rs` | 2891 | slash command domain is broad | list/show/run/review/merge/cleanup handlers |
| `src/tui/slash_handler/learning.rs` | 2742 | still mixes learning, memory proposal, improvement, recovery commands | memory proposal commands, improvement commands, recovery/feedback commands |
| `src/tui/screens/main_screen.rs` | 2730 | rendering can sprawl quickly | layout sections, message list, input/status bars |
| `src/tui/slash_handler/session.rs` | 2327 | session commands affect persistence and trace UX | session list/show/delete/export helpers |
| `src/tui/commands.rs` | 2073 | command registry and help output can become noisy | command definitions, aliases, help rendering |
| `apps/desktop/src-tauri/src/lib.rs` | 2727 | desktop bridge should stay thin over runtime | command groups, diagnostics, settings, session/run bridge |
| `apps/desktop/src/app/runEventState.ts` | 1537 | frontend runtime event reducer is important | event reducer, selectors, display transforms |
| `apps/desktop/src/runtime/desktopApi.ts` | 1279 | API bridge can hide contract drift | typed command calls, run streaming helpers, diagnostics API |
| `apps/desktop/src/app/App.tsx` | 1105 | app shell should not absorb feature logic | shell layout, session pane, run pane, diagnostics pane |

Acceptance for Band 3:

```bash
cargo fmt --check
cargo test -q tui -- --test-threads=1
(cd apps/desktop/src-tauri && cargo fmt --check && cargo test -q)
scripts/runtime-entrypoint-smoke.sh --cli --timeout 5
scripts/runtime-entrypoint-smoke.sh --tui --timeout 5
scripts/runtime-entrypoint-smoke.sh --desktop-quick
```

### Band 4: Eval, Scripts, And Reporting

These files are less risky than runtime safety boundaries, but daily baseline
maintenance depends on them staying easy to test.

| File | Lines | Reason | Recommended Boundary |
|------|-------|--------|----------------------|
| `scripts/live_eval_report_parser.py` | 2919 | parser/report code is broad | parser model, quality classification, markdown rendering |
| `scripts/run_live_eval.sh` | 2626 | shell orchestration is still long | preparation, execution, artifact collection, summary |
| `src/engine/evalset.rs` | 2554 | eval runner plus reporting still broad | external baseline reports, trend reports, replay trace helpers |
| `src/engine/context_compressor.rs` | 2521 | compression is complex and prompt-sensitive | summary parsing, truncation policy, tool-call repair |
| `src/engine/scenario_matrix.rs` | 1885 | matrix/reporting can grow with eval coverage | scenario model, run aggregation, report rendering |
| `src/memory/eval.rs` | 1598 | memory quality tests need clarity | eval model, runner, report formatting |

Acceptance for Band 4:

```bash
bash -n scripts/run_live_eval.sh scripts/product-daily-gate.sh
python3 -m py_compile scripts/live_eval_report_parser.py scripts/live_eval_quality_status.py scripts/product_daily_summary.py
bash scripts/live-eval-summary-smoke.sh
bash scripts/product-daily-gate.sh --dry-run --layer product
cargo test -q evalset -- --test-threads=1
cargo test -q context_compressor -- --test-threads=1
```

## Execution Plan

### Phase 1: Guardrails And Reporting

Goal: prevent regression while allowing gradual cleanup.

Tasks:

1. add or document a 3000-line hard check in the daily/dev gate;
2. keep 1500/1000-line reports non-blocking at first;
3. update the file-size report to optionally classify test-only files,
   frontend files, scripts, and runtime/tooling files separately;
4. add a short docs section explaining allowed exceptions.

Exit criteria:

- `scripts/file-size-report.sh --threshold 3000` stays at `files: 0`;
- size report can distinguish runtime files from test-only files;
- contributors can see which files are `priority_split` vs `watch`.

### Phase 2: Tool And Memory Safety Boundaries

Goal: split the highest-risk safety and memory surfaces before adding new
behavior.

Recommended commits:

1. split `src/memory/provider.rs` into provider trait/facade, local provider,
   static-prefix assembly, and diagnostics;
2. split `src/tools/file_tool/mod.rs` into read/search/list, write/edit,
   contracts, and rendering;
3. split `src/tools/memory_tool/mod.rs` by command family;
4. split `src/tools/bash_tool/mod.rs` and
   `src/tools/bash_tool/command_classifier.rs` by execution, background task,
   policy classification, and display helpers;
5. split `src/permissions/mod.rs` by rule model, matching, persistence, and
   review summaries.

Exit criteria:

- no Band 1 runtime file remains over 2000 lines without a documented reason;
- file, bash, memory, and permission tests pass;
- no permission/checkpoint behavior is weakened.

### Phase 3: Runtime Repair Loop Boundaries

Goal: make complex repair loops easier to diagnose and change.

Recommended commits:

1. split patch recovery into diagnostics, recovery-plan selection, and
   synthesis handoff;
2. split tool execution controller into preflight, dispatch, observation, and
   repair feedback;
3. split request preparation into context assembly, tool exposure, and budget
   accounting;
4. split tool-result handling into normalization, duplicate suppression, and
   repair signal extraction;
5. split evidence/workflow/task context reporting helpers where they obscure
   policy decisions.

Exit criteria:

- repair-loop related files trend below 2000 lines;
- live eval failures still classify honestly as framework/model/harness/user;
- closeout and runtime-spine tests remain green.

### Phase 4: TUI And Desktop Real-Path Cleanup

Goal: keep user-facing paths maintainable without redesigning the UI.

Recommended commits:

1. split `src/tui/app.rs` below 2000 lines by moving remaining state/input
   helpers into child modules;
2. split large slash handlers by command family;
3. split `main_screen.rs` into stable rendering sections;
4. split desktop Tauri `lib.rs` by command group while preserving the thin
   bridge over `StreamingQueryEngine`;
5. split desktop frontend reducers/API helpers only where types and contracts
   become clearer.

Exit criteria:

- TUI and desktop smoke pass;
- no visual or interaction behavior changes are mixed into extraction commits;
- desktop bridge remains thin and traceable.

### Phase 5: Eval And Script Maintenance

Goal: make daily/live eval easier to evolve without fragile shell edits.

Recommended commits:

1. move more live-eval report parsing into Python modules;
2. reduce `run_live_eval.sh` to orchestration phases;
3. split evalset report rendering from replay checks;
4. keep smoke tests for parser/report helpers.

Exit criteria:

- daily dry-run and report-only flows pass;
- live-eval summary smoke passes;
- parser helpers compile independently.

## Metrics

Track these numbers after each phase:

```bash
scripts/file-size-report.sh --threshold 3000
scripts/file-size-report.sh --threshold 1500
scripts/file-size-report.sh --threshold 1000
```

Target after this next phase:

- keep files over 3000 at `0`;
- reduce files over 1500 from `45` to under `25`;
- reduce runtime/tooling files over 1000 from about `60` to under `45`;
- keep newly created runtime files under 1000 where practical;
- keep test-only exceptions documented instead of hidden in runtime modules.

## Review Checklist

Before merging each slimming slice:

- the diff is mostly move/extract, not behavior rewrite;
- public APIs did not widen unnecessarily;
- moved tests preserve raw string fixture indentation;
- narrow tests for the touched area pass;
- a broader gate runs when shared runtime/tool contracts moved;
- file-size report improves or the exception is documented.

## First Recommended Slice

Start with the guardrail/reporting slice:

1. extend `scripts/file-size-report.sh` with category output;
2. add a non-blocking 1500/1000 report mode for daily/dev gates;
3. keep the 3000-line threshold as the hard no-regression gate;
4. document test-only and generated-file exceptions;
5. validate with `scripts/file-size-report.sh --threshold 3000` and
   `cargo check -q`.

This gives immediate protection against regression before touching more runtime
files.

## Implementation Log

### 2026-06-03 Phase 1: Guardrails And Reporting

Status: completed.

Changes:

- extended `scripts/file-size-report.sh` with source categories, action labels,
  category summaries, JSON summary fields, and `--fail-over N`;
- added the 3000-line no-regression hard gate to
  `scripts/product-daily-gate.sh`;
- added the same no-regression hard gate to
  `scripts/coding-workflow-gates.sh quick`;
- kept the 1500-line report as a non-blocking watchlist.

Validation:

```bash
bash -n scripts/file-size-report.sh scripts/product-daily-gate.sh scripts/coding-workflow-gates.sh
scripts/file-size-report.sh --threshold 3000 --fail-over 3000
scripts/file-size-report.sh --threshold 1500 --top 5
scripts/file-size-report.sh --threshold 3000 --json
scripts/product-daily-gate.sh --dry-run --layer smoke
```

### 2026-06-03 Phase 2: Tool And Memory Safety Boundaries

Status: completed.

Changes:

- moved large inline tests out of Band 1 runtime files into child test modules:
  `src/memory/provider/tests.rs`, `src/tools/agent_tool/tests.rs`,
  `src/tools/memory_tool/tests.rs`, `src/tools/bash_tool/tests.rs`,
  `src/tools/tests.rs`, and `src/permissions/tests.rs`;
- split `src/tools/file_tool/mod.rs` into focused `read`, `write`, and
  `state` child modules while keeping the public file-tool API stable;
- kept read-before-write tracking, file state checks, edit previews, and diff
  rendering behind the same `file_tool` boundary;
- reduced all Band 1 target runtime files below the 2000-line warning
  threshold without weakening tool, memory, checkpoint, or permission behavior.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q file_tool -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q memory -- --test-threads=1
cargo test -q permission -- --test-threads=1
cargo test -q agent_tool -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
scripts/file-size-report.sh --threshold 2000 --top 35
```

### 2026-06-03 Phase 3: Runtime Repair Loop Boundaries

Status: completed.

Changes:

- moved inline tests out of runtime repair/control files into child test
  modules for `tool_execution_controller`, `request_preparation_controller`,
  `tool_result_controller`, `streaming`, `task_context`, and
  `workflow_contract`;
- moved deterministic patch repair rule implementations from
  `patch_recovery.rs` into `patch_repair_rules.rs`, leaving
  `patch_recovery.rs` focused on synthesis flow and patch validation;
- moved MCP local tool adapter and `/mcp` management tool code into
  `src/engine/mcp/tool.rs`, keeping `mcp.rs` focused on client/manager state;
- moved evidence-ledger tool-result summary and metadata helpers into
  `src/engine/evidence_ledger/tool_records.rs`;
- reduced all Band 2 target files below the 2000-line warning threshold.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q conversation_loop -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo test -q runtime_spine_behavior_contract_covers_context_action_progress_stop_and_proof -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q mcp -- --test-threads=1
scripts/file-size-report.sh --threshold 2000 --top 30
```

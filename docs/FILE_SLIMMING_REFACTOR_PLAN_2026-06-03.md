# File Slimming Refactor Plan

Date: 2026-06-03

Status: active implementation plan

## Summary

Priority Agent does not currently satisfy a strict "source files under 1000
lines" engineering standard.

The current codebase can run and has stronger runtime gates than before, but it
still contains many large files that mix multiple responsibilities. This is now
an engineering-quality issue because the next product phase depends on stable
repair loops, TUI/desktop entrypoints, provider slow-tail reporting, and daily
baseline maintenance. Those areas become harder to change safely when the main
modules are several thousand lines long.

The goal of this plan is not to mechanically split every long file. The goal is
to reduce high-risk coupling in the files that slow down development and make
verification harder.

## Current Size Snapshot

This snapshot excludes generated or dependency-heavy directories such as
`target/`, `node_modules/`, `dist/`, `docs/`, `evalsets/`, and benchmark
artifacts.

| Area | Files Checked | Files Over 1000 Lines | Largest File |
|------|---------------|------------------------|--------------|
| `src/` | 394 | 60 | 5607 lines |
| `apps/desktop/src` | 25 | 3 | 1537 lines |
| `apps/desktop/src-tauri/src` | 2 | 1 | 3256 lines |
| `scripts/` | 42 | 2 | 3192 lines |
| `tests/` | 8 | 0 | 217 lines |

Total over-1000 engineering files: 66.

Largest files observed:

```text
5607  src/tui/app.rs
5186  src/tools/file_tool/mod.rs
4461  src/memory/manager.rs
4298  src/engine/evalset.rs
4204  src/engine/task_contract.rs
3969  src/engine/context_compressor.rs
3704  src/engine/conversation_loop/mod.rs
3619  src/tui/slash_handler/learning.rs
3502  src/engine/evidence_ledger.rs
3256  apps/desktop/src-tauri/src/lib.rs
3192  scripts/run_live_eval.sh
2919  scripts/live_eval_report_parser.py
2910  src/memory/provider.rs
2891  src/tui/slash_handler/agents.rs
2730  src/tui/screens/main_screen.rs
2725  src/engine/conversation_loop/patch_recovery.rs
2682  src/engine/conversation_loop/tool_execution_controller.rs
2668  src/engine/mcp.rs
2520  src/tools/agent_tool/mod.rs
2357  src/tools/memory_tool/mod.rs
```

## Engineering Interpretation

A 1000-line limit is a useful heuristic, not an absolute rule.

Some files can reasonably exceed the limit:

- generated code;
- schema snapshots;
- large static data tables;
- focused parser fixtures;
- dense tests that are intentionally local to one behavior.

That is not the main issue here. Several oversized files in this repo are not
just large data containers. They combine orchestration, policy, execution,
formatting, reporting, UI state, and tests in one place.

The practical risk is:

- changes require reading too much unrelated code;
- tests become harder to target;
- behavior boundaries are unclear;
- bug fixes create incidental edits in broad files;
- future contributors and weaker agents have a harder time making safe edits;
- prompt/context budget is wasted when large modules must be inspected.

So the right response is a focused slimming program, not a broad rewrite.

## Target Standard

Use this as a soft project standard:

- ordinary source files should trend below 1000 lines;
- orchestrator files may temporarily tolerate 1000-1500 lines if they are
  mostly wiring;
- files over 1500 lines should have a documented split boundary;
- files over 3000 lines should be treated as priority refactor candidates;
- new files should not become catch-all modules.

This should not become a rigid blocker for emergency fixes. It should be a
maintenance pressure that keeps new work from adding to the worst files.

## Non-Goals

- Do not do a big-bang architecture rewrite.
- Do not split files by arbitrary line ranges.
- Do not move code without tests or compile feedback.
- Do not weaken validation, permissions, checkpointing, or closeout contracts
  during slimming.
- Do not add abstractions just to reduce line count.
- Do not make call paths harder to trace.
- Do not mix large behavior changes with mechanical extraction.

## Split Principles

Each split should preserve behavior and improve a real boundary.

Prefer extracting by responsibility:

- `types`: structs, enums, serializable records;
- `policy`: deterministic classification and gate decisions;
- `execution`: side-effecting tool/runtime execution;
- `reporting`: markdown/json/status rendering;
- `parsing`: command, trace, log, or response parsing;
- `state`: state machines and transition helpers;
- `tests`: focused unit tests near the extracted behavior;
- `fixtures`: static test data outside runtime code.

Avoid extracting:

- tiny one-function modules with no ownership boundary;
- modules that only hide dependencies but do not reduce conceptual load;
- code that forces circular dependencies or broad `pub` leakage.

## Priority Order

### 1. Live Eval And Daily Scripts

Files:

- `scripts/run_live_eval.sh`
- `scripts/live_eval_report_parser.py`
- `scripts/product-daily-gate.sh`

Why first:

- daily baseline is now the product health signal;
- repair-loop and provider slow-tail work depend on these scripts;
- long shell scripts are fragile and hard to test;
- moving reporting/parsing into Python helpers will reduce bash complexity.

Recommended split:

- keep `scripts/run_live_eval.sh` as an orchestration wrapper;
- move report/status generation into Python modules;
- move provider failure classification into a reusable parser helper;
- keep task preparation and collection as clear shell phases;
- add a small smoke test for each parser helper.

Acceptance:

```bash
bash -n scripts/run_live_eval.sh scripts/product-daily-gate.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/product-daily-gate.sh --dry-run --layer product
bash scripts/live-eval-summary-smoke.sh
```

### 2. File Tool Boundary

File:

- `src/tools/file_tool/mod.rs`

Why:

- this is a core safety boundary;
- permissions, path normalization, read/write execution, result formatting, and
  tests are currently concentrated in one large module;
- mistakes here can affect workspace safety and user trust.

Recommended split:

- `path_policy.rs`: root allow/deny checks, path normalization, runtime artifact
  read allowances;
- `read.rs`: read/list/search read-only execution;
- `write.rs`: write/edit execution boundaries;
- `contracts.rs`: parameter schemas and tool contract metadata;
- `diagnostics.rs`: existing diagnostic helpers stay separate;
- `tests.rs` or focused per-module tests.

Acceptance:

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q grep_allows_runtime_tool_result_artifacts_read_only -- --test-threads=1
```

### 2026-06-03 Slice 4: Memory Manager Files And Tests Boundary

Status: completed.

Changes:

- moved the large `MemoryManager` test module into
  `src/memory/manager/tests.rs`, keeping it as a child module so private test
  coverage remains intact;
- extracted markdown memory file loading, file manifest rendering, topic path
  inference, section maintenance, archiving, hashing, atomic writes, and file
  locks into `src/memory/files.rs`;
- updated memory persistence and retrieval modules to call the file boundary
  directly;
- reduced `src/memory/manager.rs` from 4461 lines to 2010 lines while keeping
  `MemoryManager` as the runtime facade.

Validation:

```bash
cargo fmt --check
cargo test -q memory -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
bash scripts/active-memory-baseline.sh
```

### 2026-06-03 Slice 5: Conversation Loop Test Boundary

Status: completed.

Changes:

- moved the large inline `conversation_loop` regression test module into
  `src/engine/conversation_loop/tests.rs`;
- kept it as a child module of `conversation_loop`, preserving access to
  private helpers and deterministic patch-repair fixtures;
- preserved raw string fixture indentation so deterministic patch anchors stay
  byte-for-byte meaningful;
- reduced `src/engine/conversation_loop/mod.rs` from 3704 lines to 733 lines.

Validation:

```bash
cargo fmt
cargo test -q conversation_loop -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo test -q runtime_spine_behavior_contract_covers_context_action_progress_stop_and_proof -- --test-threads=1
```

### 3. Memory Manager And Provider

Files:

- `src/memory/manager.rs`
- `src/memory/provider.rs`

Why:

- memory is a product differentiator;
- the current facade is still too broad;
- future work will touch recall, proposal review, static prefix selection, and
  memory status surfaces.

Recommended split:

- keep `MemoryManager` as a narrow facade;
- extract recall orchestration from persistence;
- extract proposal/write-policy handling;
- extract status/report rendering;
- keep provider traits small and move local-provider implementation details out
  of the trait module.

Acceptance:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
bash scripts/active-memory-baseline.sh
```

### 4. Conversation Loop Entrypoint

File:

- `src/engine/conversation_loop/mod.rs`

Why:

- this is the central runtime path;
- many controllers already exist, but the module file still carries too much
  wiring, helper behavior, and test surface;
- complex repair-loop stabilization depends on clear runtime boundaries.

Recommended split:

- keep `mod.rs` as the public assembly point;
- move eval-run helper code into a dedicated module;
- move test-only fixtures into test helpers;
- keep controller imports grouped by phase;
- avoid changing controller behavior during the first slimming pass.

Acceptance:

```bash
cargo test -q conversation_loop -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo test -q runtime_spine_behavior_contract_covers_context_action_progress_stop_and_proof -- --test-threads=1
```

### 5. TUI And Desktop Entrypoints

Files:

- `src/tui/app.rs`
- `src/tui/screens/main_screen.rs`
- `src/tui/slash_handler/learning.rs`
- `src/tui/slash_handler/agents.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/runtime/desktopApi.ts`

Why:

- user-facing entrypoints need stable, inspectable state transitions;
- UI files naturally grow, but state, rendering, commands, and bridge logic
  should not all live together;
- TUI/desktop real-path smoke is now part of the stability plan.

Recommended split:

- separate UI state from rendering widgets;
- keep command handlers grouped by domain;
- extract Tauri command groups from `lib.rs`;
- preserve the thin desktop bridge boundary over `StreamingQueryEngine`;
- avoid visual refactors while moving code.

Acceptance:

```bash
cargo test -q tui -- --test-threads=1
scripts/runtime-entrypoint-smoke.sh --cli --timeout 5
scripts/runtime-entrypoint-smoke.sh --tui --timeout 5
scripts/runtime-entrypoint-smoke.sh --desktop-quick
```

### 6. Large Engine Support Modules

Files:

- `src/engine/evalset.rs`
- `src/engine/task_contract.rs`
- `src/engine/context_compressor.rs`
- `src/engine/evidence_ledger.rs`
- `src/engine/trace.rs`
- `src/engine/workflow_contract.rs`
- `src/engine/task_context.rs`

Why:

- these modules carry important policy and reporting logic;
- they should be split after the main runtime and tool boundaries are safer;
- premature extraction here could create broad API churn.

Recommended split:

- extract serializable record types;
- extract renderers/reporters;
- extract deterministic policy rules;
- keep public APIs stable until tests prove behavior is unchanged.

Acceptance:

```bash
cargo test -q evalset -- --test-threads=1
cargo test -q task_contract -- --test-threads=1
cargo test -q context_compressor -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
```

## Execution Strategy

Use small, boring refactor slices.

For each oversized file:

1. identify responsibilities and current tests;
2. create a before/after file-size target;
3. extract one responsibility with no behavior change;
4. run the narrowest tests;
5. run a broader gate only when a shared contract moved;
6. commit the slice before moving to the next file.

Recommended slice size:

- one file family at a time;
- one responsibility per commit;
- no semantic behavior changes unless a test exposes a bug;
- no mixed UI polish, provider logic, and memory changes in the same commit.

## Measurement

Add a lightweight size-report script before the refactor grows:

```bash
scripts/file-size-report.sh --top 30
scripts/file-size-report.sh --threshold 1000
```

Suggested report fields:

- file path;
- line count;
- area;
- owner category;
- generated/dependency excluded status;
- recommended action.

The script should exclude at least:

- `.git/`;
- `target/`;
- `node_modules/`;
- `dist/`;
- generated benchmark outputs;
- vendored or generated schemas unless explicitly requested.

## Success Criteria

This slimming program is successful when:

- top 10 oversized files have an explicit split plan or are reduced;
- no ordinary runtime/tool/UI source file remains over 3000 lines without a
  documented exception;
- daily and live-eval scripts are easier to test and review;
- file tool and memory boundaries are clearer;
- TUI and desktop entrypoint code is easier to inspect;
- broad validation remains green after each slice.

Recommended near-term numeric target:

- reduce files over 3000 lines from 11 to 0-2;
- reduce files over 1000 lines from 66 to under 35;
- keep all new source files below 1000 lines unless there is a documented
  exception.

## First Concrete Slice

Start with measurement and daily-script slimming:

1. add `scripts/file-size-report.sh`;
2. move product-daily summary generation out of `scripts/product-daily-gate.sh`
   into a Python helper;
3. move live-eval quality status inference out of the long bash heredoc into a
   parser helper;
4. keep shell scripts as orchestration only;
5. validate with daily dry-run, report-only, and summary smoke.

This gives immediate value without touching the core Rust runtime first.

## Implementation Log

### 2026-06-03 Slice 1: Measurement And Product Daily Summary

Status: completed.

Changes:

- added `scripts/file-size-report.sh`;
- extracted product daily summary generation from
  `scripts/product-daily-gate.sh` into `scripts/product_daily_summary.py`;
- reduced `scripts/product-daily-gate.sh` to orchestration, case selection, and
  command execution;
- kept old report-only compatibility for existing daily run artifacts.

Validation:

```bash
bash -n scripts/product-daily-gate.sh scripts/file-size-report.sh scripts/run_live_eval.sh
python3 -m py_compile scripts/product_daily_summary.py scripts/live_eval_report_parser.py
scripts/file-size-report.sh --threshold 3000
bash scripts/product-daily-gate.sh --dry-run --layer product
bash scripts/product-daily-gate.sh --report-only product-daily-tui-20260603-093401
```

### 2026-06-03 Slice 2: Live Eval Quality Status Extraction

Status: completed.

Changes:

- extracted the quality-signal and `failure_owner` generation heredoc from
  `scripts/run_live_eval.sh` into `scripts/live_eval_quality_status.py`;
- preserved the existing command-line arguments, stdout report text, and
  `agent-quality-status.txt` output format;
- reduced `scripts/run_live_eval.sh` by another large embedded Python block.

Validation:

```bash
bash -n scripts/run_live_eval.sh scripts/product-daily-gate.sh
python3 -m py_compile scripts/live_eval_quality_status.py scripts/product_daily_summary.py scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
bash scripts/product-daily-gate.sh --dry-run --layer product
bash scripts/product-daily-gate.sh --report-only product-daily-tui-20260603-093401
scripts/file-size-report.sh --top 20
```

### 2026-06-03 Slice 3: File Tool Path Policy Extraction

Status: completed.

Changes:

- extracted file-tool path resolution, normalization, absolute path allowlist,
  read-only root policy, and UNC/network-path detection into
  `src/tools/file_tool/path_policy.rs`;
- kept `src/tools/file_tool/mod.rs` as the public compatibility surface via
  re-exports, so existing callers still use `crate::tools::file_tool::*`;
- did not change file read/write behavior or permission policy.

Validation:

```bash
cargo fmt
cargo test -q file_tool -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q grep_allows_runtime_tool_result_artifacts_read_only -- --test-threads=1
```

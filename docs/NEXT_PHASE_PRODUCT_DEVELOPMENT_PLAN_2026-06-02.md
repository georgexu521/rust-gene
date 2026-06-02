# Next Phase Product Development Plan

Date: 2026-06-02

Status: proposed plan

## Summary

Priority Agent should enter a product consolidation phase.

The next phase should not chase broad generic-agent parity. Codex will keep
winning the generic coding-agent entrypoint, and Reasonix is already sharply
optimized around DeepSeek prefix-cache economics. Priority Agent's durable
advantage is different:

> A local, personal, verifiable coding agent that knows gex's machine,
> projects, habits, memory, validation loops, and risk tolerance.

The next development target is therefore:

> Make the personal-verifiable loop visible, controllable, and reliable enough
> for daily real coding work.

This plan turns that into five implementation tracks:

1. daily-use evaluation loop;
2. memory productization;
3. evidence-first CLI/TUI and desktop surfaces;
4. real coding repair reliability;
5. runtime simplification and maintainability.

## Current Diagnosis

### What Is Already Strong

- The shared runtime has a clear boundary: the LLM owns semantic and engineering
  judgment, while the runtime owns context assembly, tool execution, evidence,
  permissions, validation proof, and closeout gates.
- The runtime diet work removed several pseudo-intelligent loop branches. The
  current loop contract is easier to reason about: valid no-tool assistant
  response finishes the turn, empty responses get bounded retry, exact repeated
  tool storms are bounded, and hard safety/verification constraints remain.
- Memory has moved past a vague "agent remembers things" shape. It now has
  pinned memory, dynamic recall, and review-first proposals.
- Route-scoped tools, prompt-cache diagnostics, `/doctor`, `/cache
  miss-report`, `/memory control`, `/memory-proposals`, `/active-task`, and
  desktop workbench snapshots are already present enough to become product
  surfaces.
- Eval infrastructure is richer than most personal agents: runtime-spine events,
  live eval reports, run bundles, behavior assertions, and real-project task
  plans already exist.

### What Is Still Weak

- The user-facing product story is still buried in diagnostics. The runtime can
  produce good evidence, but the everyday surface does not yet make the state
  obvious at a glance.
- Memory controls exist, but the default daily workflow is not yet simple enough:
  users need to understand when memory is used, why a memory was selected, what
  is proposed, and what will become durable.
- Active memory is still an opt-in prototype. It is correctly conservative, but
  the project needs a decision on whether it becomes a normal controlled
  session feature.
- TUI polish has improved, but Reasonix still leads in live activity feedback,
  tool cards, streaming metadata, status/cost/cache display, toast
  notifications, and compact card detail.
- The codebase remains complex in the places that matter most. As of this
  audit, `src/memory/manager.rs`, `src/memory/provider.rs`, and the
  conversation-loop modules are still large enough that feature work can easily
  add more coupling.
- Several plans describe evals and real-world tests, but the next daily
  scoreboard is not yet defined as the product gate.

## Product Thesis For The Next Phase

Priority Agent should be developed as a "personal coding workbench", not as a
generic terminal chatbot.

The user should be able to answer these questions at any moment:

- What is the agent doing now?
- Which context, memory, tools, and route did it choose?
- What changed in the workspace?
- What evidence proves the change?
- What failed, and what repair path is being attempted?
- What memory or skill update is being proposed for future work?
- Is the final answer verified, partial, failed, or not verified?

If the product answers those questions clearly, it has a real distinction from
both Codex and Reasonix.

## Non-Goals

- Do not add broad new tool categories just to match competitors.
- Do not add more always-on prompt rules for one-off model mistakes.
- Do not weaken permissions, checkpoints, validation, or honest closeout to
  make weak providers look better.
- Do not make active memory an invisible hidden planner.
- Do not keep expanding `MemoryManager` or `conversation_loop::mod` when a
  small extracted module would preserve the same behavior.
- Do not treat desktop, TUI, and eval-run as separate runtimes. They should keep
  sharing `StreamingQueryEngine`.

## Track 1: Daily-Use Evaluation Loop

Goal: make every product slice prove itself on realistic coding work.

### 1.1 Create A Small Daily Scoreboard

Create a compact scoreboard from existing evals instead of running every suite
for every change.

Recommended daily cases:

- `core-inspection-grounding`
- `core-simple-stale-edit`
- `core-multi-file-edit`
- `core-rust-multi-file-refactor`
- `code-change-verification-repair-loop`
- `project-partner-resume-with-memory`
- `memory-recall-conflict-precision`
- `minimum-agent-verification-repair`
- `desktop-ui-smoke-polish`

Acceptance:

- one command runs the daily set;
- report includes outcome status, proof status, closeout status, tool failures,
  memory proposal status, prompt-cache miss reason, and runtime-spine phase
  coverage;
- failure output tells whether the owner is model behavior, runtime flow,
  harness/setup, provider protocol, or UX.

Likely implementation:

- add `scripts/product-daily-gate.sh`;
- add `evalsets/product_daily.yaml` or a named list consumed by
  `scripts/run_live_eval.sh`;
- add a short `docs/PRODUCT_DAILY_SCOREBOARD_2026-06-02.md` after the first
  baseline run.

Validation:

```bash
bash -n scripts/product-daily-gate.sh
bash scripts/product-daily-gate.sh --dry-run
python3 -m py_compile scripts/live_eval_report_parser.py
cargo test -q scenario_matrix
```

### 1.2 Make Real-Project Coding The Main Regression Gate

The existing `LIVE_CODING_TEST_PLAN_2026-06-01.md` and
`REAL_WORLD_TEST_PLAN_2026-06-01.md` are useful, but they should become a
single tracked capability ladder.

Recommended ladder:

- Level 1: inspect and explain;
- Level 2: one-file bug fix with test;
- Level 3: stale edit repair;
- Level 4: multi-file refactor;
- Level 5: validation failure repair loop;
- Level 6: long task with force-summary and honest partial closeout.

Acceptance:

- each level has one stable prompt, one expected behavior contract, and one
  report parser assertion;
- a failed validation command must re-enter the model context as evidence;
- passing a task without proof is not a green run.

## Track 2: Memory Productization

Goal: make memory predictable, inspectable, and safe enough to trust daily.

### 2.1 Finish The User-Control Model

Current controls:

- `/memory control use on|off`
- `/memory control generate on|off`
- `/memory control recall off|strict|balanced|preference-only`

Next controls to consider:

- `/memory control write-policy review-only|narrow|legacy`
- `/memory control active off|on`
- `/memory status`

Recommended default:

- `use=on`
- `generate=on`
- `recall=balanced`
- `write-policy=review-only`
- `active=off` until Track 2.3 is accepted

Acceptance:

- `/memory status` explains current controls in user language;
- `/memory status --json` exposes machine-readable state;
- `/doctor` includes the same state and the reason when memory is skipped;
- all controls stay session-scoped unless explicitly made persistent.

Validation:

```bash
cargo test -q memory_use_and_generate_controls_are_independent
cargo test -q memory
cargo test -q doctor_prompt_cache_line_reports_tool_schema_miss_reason
```

### 2.2 Make Memory Selection Explainable

The user needs to understand why a memory entered the current turn.

Improve `/memory why` into a product-level answer:

- show query used;
- show source category: pinned, recall, active-memory, proposal, project file;
- show score or mode reason;
- show whether it affected stable prefix or dynamic user-tail context;
- show whether it was filtered out and why.

Acceptance:

- `/memory why <query>` answers with selected and rejected items;
- `/memory why --last-turn` uses the latest actual turn query and retrieval
  trace;
- active-memory results are clearly labelled as untrusted dynamic context;
- strict/preference-only/off modes have visible explanations.

### 2.3 Decide Whether Active Memory Graduates

Active memory is currently a good prototype: gated, local, read-only, bounded,
and skipped for eval/headless/automation/internal paths.

Before making it normal:

- collect a baseline on persistent interactive sessions;
- measure added latency and prompt-cache impact;
- verify that empty/timeout/failure statuses remain non-blocking;
- ensure active-memory items cannot become durable memory without the proposal
  review path.

Graduation rule:

- if it improves at least two memory evals or real resume tasks without causing
  cache instability or confusing user-facing answers, expose it as
  `/memory control active on`;
- otherwise keep it opt-in via `PRIORITY_AGENT_ACTIVE_MEMORY=1`.

Validation:

```bash
cargo test -q active_memory
cargo test -q turn_retrieval_context_controller
cargo test -q request_preparation_controller
```

### 2.4 Continue Shrinking MemoryManager

`src/memory/manager.rs` is still a large coordination file. Continue extracting
only low-coupling slices.

Preferred next extractions:

- proposal queue commands and formatting;
- snapshot loading and safety filtering;
- provider lifecycle diagnostics;
- memory doctor report assembly.

Acceptance:

- no behavior change in the first extraction slice;
- `MemoryManager` remains the facade, but pure helpers move out;
- tests stay at the touched-module level first.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
bash scripts/test-fast-lane.sh
```

## Track 3: Evidence-First UI And Workbench

Goal: make the runtime's evidence visible without making the user read trace
logs.

### 3.1 TUI Live Activity Area

Reasonix is stronger here. Priority Agent should add a compact live area above
the composer or status bar.

Show:

- current model/thinking state;
- current running tool;
- tool args summary;
- elapsed time;
- validation command status;
- current phase: context, decision, tool, repair, validation, closeout;
- memory recall/proposal indicator when active.

Acceptance:

- long-running bash/tool calls are visible before completion;
- the live area collapses when idle;
- it does not permanently add noisy messages to the transcript;
- it uses existing runtime events instead of inventing a second state machine.

Validation:

```bash
cargo test -q tui
bash scripts/tui-dogfood-test.sh
```

### 3.2 Tool Cards That Explain What Happened

Upgrade TUI tool rendering before adding more commands.

Priority tool cards:

- bash: command, exit code, elapsed, last lines, failure reason;
- file edit/patch/write: path, changed lines, checkpoint id, diff summary;
- file read/grep/glob: path/query, result count, truncation/artifact state;
- memory: query, selected items, write policy, proposal id.

Acceptance:

- the user can scan a run without opening `/trace`;
- failed tools have an actionable reason;
- checkpointed file changes are visible at the tool card level;
- repeated/truncated outputs show compact summaries.

### 3.3 Status Bar And Turn Summary

TUI and desktop should expose the same product metrics:

- provider/model;
- running/idle;
- context usage;
- prompt-cache hit rate and last miss reason;
- turn cost or token estimate when available;
- memory use/generate/recall mode;
- verification/closeout status for the latest turn.

Desktop already has some of this in `StatusBar.tsx` and `WorkbenchPanel.tsx`.
The next task is consistency between surfaces.

Acceptance:

- TUI and desktop show the same cache/context concepts;
- `/doctor` remains the deep version, not the only way to see state;
- status bar stays responsive and compact.

### 3.4 Desktop Workbench As The Personal Agent Surface

The desktop workbench should not become decorative. It should be the place
where project intelligence and runtime evidence are inspectable.

Next desktop panels:

- active task summary;
- memory status and proposal queue;
- latest validation proof;
- prompt-cache and context detail;
- recent changed files and checkpoints;
- doctor summary with fix suggestions.

Acceptance:

- no duplicate runtime: desktop consumes the same `StreamingQueryEngine`
  signals;
- Playwright smoke verifies layout and non-overlap;
- a user can inspect why a run was partial/failed without reading raw logs.

Validation:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

## Track 4: Real Coding Repair Reliability

Goal: improve actual code-change success, not just diagnostics.

### 4.1 Required Validation Repair Loop

Focus on the loop:

```text
tool or validation failure
-> structured ToolObservation
-> next model context
-> bounded repair
-> required validation proof
-> honest closeout
```

Acceptance:

- failed validation output includes enough source/error snippet for repair;
- repeated failed validation attempts are traceable;
- final answer cannot claim verified success without proof;
- partial/failure closeout is clear and useful.

Validation:

```bash
cargo test -q closeout
cargo test -q validation_runner
cargo test -q repair_controller
bash scripts/runtime-spine-fast-gate.sh
```

### 4.2 Stale Edit And Read-Before-Edit Reliability

This is a daily coding pain point and should stay high priority.

Acceptance:

- edits against stale reads produce a clear re-read/repair path;
- exact duplicate reads are bounded, but changed path/offset/limit reads remain
  allowed;
- failed edits feed exact old/new string context back to the model.

Validation:

```bash
cargo test -q file_tool
cargo test -q tool_execution_controller
cargo test -q core-simple-stale-edit
```

### 4.3 Patch And Diff Ergonomics

Priority Agent should make edits feel auditable:

- checkpoint id visible;
- diff summary visible;
- rollback target obvious;
- failed patch reason clear;
- no raw bash workspace mutation bypassing checkpoint-managed tools.

Acceptance:

- every mutating file tool produces enough evidence for `/active-task`;
- rollback and checkpoint records are connected to tool cards;
- action review still blocks unsafe raw bash mutation.

## Track 5: Runtime Simplicity And Maintainability

Goal: keep product improvements from re-growing hidden runtime complexity.

### 5.1 Keep The Loop Contract Small

When adding a new behavior, classify it:

- hard contract: permission, path safety, checkpoint, validation, provider
  protocol, destructive scope, budget;
- evidence: trace, observation, report, doctor line;
- LLM context: recent observation, retrieval material, user-tail dynamic
  context;
- UI only: card, toast, panel, status line.

If it is not a hard contract, avoid making it a loop-controlling branch.

Acceptance:

- new runtime branches have explicit contract category;
- no "shadow planner" logic based on soft scores alone;
- runtime hints are fenced as recent observations, not authority.

### 5.2 Continue Route-Scoped Tool And Cache Stability Work

Reasonix's cache-first lesson still matters, but Priority Agent should apply it
provider-neutrally.

Acceptance:

- dynamic task focus, retrieval, and runtime corrections stay out of the stable
  prefix in the main request path;
- route-scoped tool schema fingerprints are visible in `/doctor`;
- cache miss reasons remain explainable.

Validation:

```bash
cargo test -q cache_stability
cargo test -q route_scoped_tools
cargo test -q request_preparation_controller
```

### 5.3 Maintain Module Boundaries

Avoid another broad rewrite. Use small, gateable slices.

Good targets:

- `src/memory/manager.rs`
- `src/memory/provider.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`

Acceptance:

- each extraction has a before/after responsibility note;
- no behavior change unless the slice explicitly says so;
- touched-module tests pass before broad tests.

## Suggested Implementation Order

### Phase 0: Baseline And Scoreboard

Deliverables:

- `scripts/product-daily-gate.sh`;
- daily eval case list;
- first baseline report;
- short note linking failures to next slices.

Why first:

- without a daily gate, UI and memory polish can look good but regress real
  coding behavior.

### Phase 1: Memory Control And Explainability

Deliverables:

- `/memory status`;
- improved `/memory why`;
- optional `write-policy` control if we decide to expose it;
- active-memory graduation decision.

Why second:

- memory is the core personal-agent differentiator, but it must be predictable
  before it becomes more active.

### Phase 2: TUI Evidence Surface

Deliverables:

- live activity area;
- stronger tool cards;
- status bar with cache/context/memory/proof indicators;
- toast or transient notification rail for non-critical system messages.

Why third:

- daily use depends on seeing what the agent is doing, not just reading final
  answers.

### Phase 3: Desktop Workbench Evidence Surface

Deliverables:

- active task panel;
- memory/proposal panel;
- validation proof panel;
- cache/context detail panel;
- Playwright smoke coverage.

Why fourth:

- desktop should become the rich inspection surface after TUI concepts are
  settled.

### Phase 4: Real Coding Repair Hardening

Deliverables:

- stale edit repair improvements;
- validation failure repair improvements;
- patch/diff/checkpoint evidence improvements;
- product daily gate trend report.

Why fifth:

- after the surfaces expose failures clearly, repair work can target observed
  failure modes instead of guesses.

### Phase 5: Maintainability Cleanup

Deliverables:

- one or two low-risk `MemoryManager` extractions;
- one conversation-loop extraction or simplification;
- updated `PROJECT_MAP.md` if module boundaries change;
- focused tests after each slice.

Why last:

- cleanup should follow concrete product slices, so it removes real complexity
  rather than creating abstractions in advance.

## Success Metrics

### Product Metrics

- A normal coding run shows current action, evidence, memory state, and
  verification status without opening raw trace logs.
- `/memory status` and `/memory why` make memory behavior understandable to a
  non-implementer.
- `/active-task` and desktop workbench explain partial/failure states clearly.
- Prompt-cache misses have visible reasons.

### Engineering Metrics

- Daily product gate is runnable with one command.
- Touched-module tests remain fast enough to use during development.
- `MemoryManager` and conversation-loop responsibilities shrink in small
  verified slices.
- No new always-on prompt bloat is added for one-off mistakes.

### Reliability Metrics

- Required validation failures re-enter context and trigger bounded repair.
- False green closeout remains blocked.
- Exact duplicate tool storms are contained without blocking legitimate
  multi-range reads.
- Memory proposals default to review-first unless explicitly narrowed by policy.

## Recommended First Commit

The first code commit after this plan should be small:

1. add `scripts/product-daily-gate.sh` with dry-run support;
2. define the daily eval case list;
3. add a report template;
4. run shell syntax and parser smoke tests.

That gives the next phase a measurable baseline before UI or memory changes.

## Working Validation Set

Use narrow gates first, then broaden when shared runtime contracts move:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
cargo test -q cache_stability
cargo test -q request_preparation_controller
cargo test -q route_scoped_tools
cargo test -q closeout
bash scripts/test-fast-lane.sh
bash scripts/runtime-spine-fast-gate.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Run the full `cargo test -q` and desktop/Tauri gates when a slice touches shared
runtime contracts, desktop bridge behavior, or provider request shape.

# Next Phase Reasonix Alignment Plan

Date: 2026-06-04

Status: implemented and under Phase 8 stewardship

Implementation note, 2026-06-04:

- Phase 0: runtime boundary map added to `docs/PROJECT_MAP.md`.
- Phase 1: `src/engine/runtime_controller.rs` is the full-agent command/event
  boundary; desktop and TUI full-agent turns now enter through it.
- Phase 2: cache-stable prefix regression coverage expanded in
  `src/engine/cache_stability.rs`.
- Phase 3: default/route-scoped tool surface snapshots added in
  `src/engine/conversation_loop/route_scoped_tools_tests.rs`.
- Phase 4: `/doctor` includes home-store memory diagnostics aligned with the
  existing `~/.priority-agent` memory root.
- Phase 5: permission/checkpoint safety tests expanded.
- Phase 6: closeout status is a first-class `StreamEvent`, `TurnEvent`, and
  `DesktopRunEvent`.
- Phase 7: deterministic daily baseline script added at
  `scripts/daily-baseline.sh`.
- Phase 8: file-size stewardship remains ongoing by design.

## 1. Purpose

This plan records the next development phase after the current code-size and
runtime cleanup work.

The goal is not to copy Reasonix or remove Priority Agent's stronger runtime
capabilities. The goal is to learn from the parts where Reasonix is clearer:
one frontend-neutral runtime controller, cache-stable prompt assembly, a small
default tool surface, simple memory product semantics, and clean
permission/checkpoint boundaries.

Priority Agent already has stronger evidence flow, richer memory gates,
provider diagnostics, route-scoped tooling, desktop state, and daily-eval
infrastructure. The next phase should make those strengths easier to operate
and harder to regress.

## 2. Evidence Reviewed

Reasonix source reviewed under:

- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/REASONIX.md`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/CONTRIBUTING.md`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/boot/boot.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/control/controller.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/control/input.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/agent.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/tool/tool.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/tool/builtin/readfile.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/memory/memory.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/memory/store.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/permission/permission.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/checkpoint/checkpoint.go`

Priority Agent source reviewed in this pass:

- `src/engine/streaming.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/runtime_facade.rs`
- `src/engine/context_assembly.rs`
- `src/engine/cache_stability.rs`
- `src/engine/query_engine.rs`
- `src/desktop_runtime/mod.rs`
- `src/tools/mod.rs`
- `src/tools/file_tool/mod.rs`
- `src/memory/manager/mod.rs`
- `src/tui/app.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/app/runEventState.ts`
- `docs/MEMORY_PRODUCT_CONTROL_PLAN_2026-06-02.md`

The current size report also matters. At threshold 1200 there are still 41
files to watch, but no normal production Rust file is currently in the old
multi-thousand-line state. The largest remaining production surfaces are mostly
runtime/tool/TUI/desktop glue around 1400-1489 lines, plus scripts and test
exceptions.

## 3. Current Assessment

Priority Agent is no longer in the earlier "too large to reason about" state.
The project has been slimmed enough that the next risk is not file length alone.
The current risk is boundary ambiguity:

- `StreamingQueryEngine` and `ConversationLoop` are powerful, but they still
  look like the runtime itself rather than a simple frontend-neutral product
  controller.
- `desktop_runtime` is a useful boundary, but it still exposes both full agent
  turns and lightweight direct-provider turns. That is acceptable only if the
  lightweight lane is explicitly treated as a non-agent lane.
- Stable-prefix/dynamic-tail architecture exists and is much better than before,
  but it needs product-level regression tests that prove toggles, memory recall,
  provider warnings, task focus, and background notes do not mutate the cached
  prefix.
- Tool exposure is already route-aware, but the default surface is still broad
  compared with Reasonix's small built-in tool list.
- Memory is more advanced than Reasonix, but its user-facing semantics are less
  simple. We need a crisp answer to: what is remembered, where it is stored, when
  it enters the stable prefix, and when it rides the dynamic tail.

The next phase should therefore be a boundary/productization phase, not a new
feature phase.

## 4. Reasonix Lessons To Adopt

### 4.1 One Runtime Controller

Reasonix has a clear `control.Controller`:

- frontends call commands such as Send, Cancel, Approve, SetPlanMode, Compact,
  and NewSession;
- frontends render typed events;
- frontends do not re-implement turn lifecycle, approval, cancellation, or
  memory injection;
- `boot.Build` is the one assembly path that loads config, provider, memory,
  skills, tools, plugins, permissions, and checkpoints.

Priority Agent has the pieces, but they are spread across `StreamingQueryEngine`,
`ConversationLoop`, `runtime_facade`, `desktop_runtime`, TUI code, and Tauri
commands. The next phase should make the command/event boundary the product
contract.

### 4.2 Cache-First Prompt Boundary

Reasonix keeps the base prompt, tools, memory prefix, and skill index byte-stable
for the session. Plan mode, memory updates, and background jobs ride in the next
user turn through `control.Compose`; they do not mutate the system prefix.

Priority Agent already has `ContextZoneName::StablePrefix`, `dynamic_tail`, cache
diagnostics, prompt-cache miss reports, and tests around static prefix behavior.
The next step is to make this an explicit invariant with broad regression tests.

### 4.3 Small Default Tool Surface

Reasonix's tool interface is intentionally small: name, description, schema,
execute, read-only, plus optional preview for writers. Built-ins are focused
files such as read, write, edit, grep, glob, ls, bash, todo, and complete-step.

Priority Agent has more tools and richer safety metadata. That is useful, but
the default exposed tool set should feel as small as Reasonix. Advanced tools
should be route-scoped, slash-invoked, or capability-gated.

### 4.4 Simple Memory Product Semantics

Reasonix memory is easy to explain:

- standing docs load into the cached prefix at session start;
- saved memory is indexed by project and folded into the prefix next session;
- mid-session memory edits ride the next turn tail and join the prefix later;
- `# <note>` and `/remember <note>` provide a simple quick-add path;
- detailed memory files are linked instead of dumping everything into prompt.

Priority Agent's memory is more capable: frozen snapshots, proposal review,
typed records, quality gates, provider adapters, recall modes, diagnostics, and
doctor output. The next phase should keep these controls while presenting them
as a simple three-layer product.

### 4.5 Pure Permission And Checkpoint Cores

Reasonix separates permission policy from UI approval and uses snapshot-based
checkpoints for previewable writer tools. Priority Agent already has stronger
permission/checkpoint behavior, but the core should stay easy to test as pure
decision logic.

## 5. Product Principles For This Phase

1. Do not add broad new features until the runtime path is boring.
2. Keep the stable prefix stable by default; dynamic context must ride the turn
   tail or user-message lane.
3. Keep one frontend-neutral command/event boundary behind TUI and desktop.
4. Keep default tools small; expand only by route, slash command, explicit
   capability, or model-requested need.
5. Keep memory useful but explainable: standing docs, saved index, dynamic
   recall.
6. Preserve Priority Agent's hard strengths: evidence ledger, validation gates,
   repair feedback, permission gates, checkpoints, and honest closeout.
7. Use small, gateable slices. Do not attempt a broad rewrite of the agent loop.

## 6. Phase 0: Runtime Boundary Audit

Goal: produce an exact map of how user input reaches the runtime today.

Work:

- Map every frontend entry into full agent execution:
  - TUI submit path in `src/tui/app.rs`;
  - desktop full turn path in `src/desktop_runtime/mod.rs`;
  - Tauri command path in `apps/desktop/src-tauri/src/lib.rs`;
  - non-streaming CLI/query path in `src/engine/query_engine.rs`;
  - direct `ConversationLoop` or `StreamingQueryEngine` tests that represent
    runtime contracts.
- Classify each path as:
  - full agent lane;
  - lightweight non-agent lane;
  - diagnostics-only lane;
  - test-only lane.
- Identify any frontend code that performs runtime decisions that should be
  emitted by the runtime instead.
- Update the project map if current docs no longer describe the real entry
  points.

Suggested outputs:

- New or updated runtime boundary section in `docs/PROJECT_MAP.md`.
- A short entry in `docs/PROJECT_STATUS.md` if the boundary contract changes.

Acceptance:

- There is a documented command/event map for TUI and desktop.
- Lightweight desktop turns are explicitly documented as non-agent turns.
- No source changes are needed yet unless the audit finds an easy direct
  boundary violation.
- Run:

```bash
cargo check -q
```

## 7. Phase 1: Frontend-Neutral Runtime Controller

Goal: make one product controller/facade the canonical way frontends drive agent
work.

Reasonix reference:

- `internal/control/controller.go`
- `internal/boot/boot.go`

Priority Agent target:

- `src/engine/runtime_facade.rs`
- `src/desktop_runtime/mod.rs`
- `src/engine/streaming.rs`
- `src/tui/app.rs`
- `apps/desktop/src-tauri/src/lib.rs`

Work:

- Introduce or clarify a `RuntimeController` concept around the existing engine.
  It does not need a full rewrite. It can begin as a typed wrapper over
  `StreamingQueryEngine`.
- Define command methods that are stable product APIs:
  - submit full turn;
  - cancel;
  - approve or deny pending action;
  - compact;
  - new or restore session;
  - update plan mode or read-only mode;
  - context snapshot;
  - provider/runtime diagnostics snapshot.
- Define event types that frontends render without re-deriving runtime meaning:
  - assistant text delta;
  - tool started/result;
  - approval requested;
  - runtime diagnostic;
  - usage/cache update;
  - turn completed;
  - verified/partial/not-verified closeout.
- Keep `StreamingQueryEngine` as the internal executor for now. The first win is
  the boundary, not moving all loop code.
- Make TUI and desktop use the same full-turn lane. Leave lightweight chat lanes
  available only behind an explicit name such as "lightweight provider reply" or
  "non-agent quick reply".

Acceptance:

- TUI and desktop full-agent paths call the same controller/facade method.
- Desktop lightweight mode cannot be confused with the full agent lane in code
  comments, event names, or UI diagnostics.
- Runtime lifecycle state remains shared through `runtime_facade`.
- Run:

```bash
cargo check -q
cargo test -q runtime_facade
cargo test -q desktop_runtime
```

If exact test filters are not available after the slice, use the closest module
tests and record the replacement command in the implementation note.

## 8. Phase 2: Cache-Stable Prefix Contract

Goal: make prompt-cache stability an enforceable product contract.

Reasonix reference:

- `internal/control/input.go`
- `internal/agent/cachehit_e2e_test.go`
- `internal/memory/memory.go`

Priority Agent target:

- `src/engine/context_assembly.rs`
- `src/engine/cache_stability.rs`
- `src/engine/prompt_context.rs`
- `src/engine/query_engine.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/streaming/turn_messages.rs`
- `docs/MEMORY_PRODUCT_CONTROL_PLAN_2026-06-02.md`

Work:

- Add or strengthen tests proving the stable prefix fingerprint does not change
  when these dynamic conditions change:
  - plan/read-only mode;
  - current task focus;
  - recent tool failure;
  - focused repair hint;
  - dynamic memory recall result;
  - mid-session memory update notice;
  - provider slow-warning or retry notice;
  - background job completion notice;
  - closeout repair feedback.
- Ensure unexpected system messages are reported as cache risk unless they are
  part of the original session-frozen prefix.
- Keep compact memory indexes in the stable prefix only when they are frozen at
  session start.
- Keep large memory bodies, retrieved snippets, recent observations, repair
  hints, and provider diagnostics in dynamic zones.
- Add a focused cache-shape regression test similar in spirit to Reasonix's
  cache-hit E2E tests, using a mock provider or request-capture harness.

Acceptance:

- Tests fail if dynamic material is inserted into the system prompt after
  session start.
- `/cache miss-report` and `/doctor` still distinguish system, tools,
  dynamic-tail, and provider/TTL style misses.
- Run:

```bash
cargo test -q prompt_context
cargo test -q request_preparation_controller
cargo test -q cache_stability
cargo check -q
```

## 9. Phase 3: Default Tool Surface Diet

Goal: keep Priority Agent powerful, but expose fewer tools by default.

Reasonix reference:

- `internal/tool/tool.go`
- `internal/tool/builtin/*.go`

Priority Agent target:

- `src/tools/mod.rs`
- `src/tools/registry.rs`
- `src/tools/tool_exposure.rs`
- `src/engine/conversation_loop/tool_exposure_plan.rs`
- `src/tui/slash_handler/agents.rs`

Work:

- Define the default agent tool profile as the smallest practical set:
  - file read;
  - file edit/write/patch;
  - bash with permission classification;
  - glob/grep/list style discovery;
  - todo or plan tracking;
  - ask/approval where needed;
  - complete/closeout proof where needed.
- Move advanced tools behind route or explicit need:
  - browser;
  - GitHub;
  - MCP/plugin management;
  - worktree orchestration;
  - refactor/workbench helpers;
  - memory admin tools;
  - benchmark/eval tools;
  - desktop-only tools.
- Keep tool schemas sorted and canonicalized before provider requests.
- Add a tool-surface snapshot test for the default route and key specialized
  routes.
- Keep diagnostics visible so the user can see why a tool was or was not exposed.

Acceptance:

- Default route tool schema is materially smaller than the current broad
  surface.
- Specialized routes still expose the tools they need.
- Prompt-cache diagnostics report tool-schema fingerprint changes.
- Run:

```bash
cargo test -q route_scoped_tools
cargo test -q tool_exposure
cargo test -q prompt_context
cargo check -q
```

## 10. Phase 4: Memory Productization

Goal: make memory behavior easy to explain and stable in cache terms.

Reasonix reference:

- `internal/memory/memory.go`
- `internal/memory/store.go`
- `internal/control/input.go`

Priority Agent target:

- `src/memory/manager/mod.rs`
- `src/memory/manager/tests.rs`
- `src/tools/memory_tool`
- `src/engine/conversation_loop/memory_sync_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/tui/slash_handler/memory.rs`
- `docs/MEMORY_PRODUCT_CONTROL_PLAN_2026-06-02.md`

Product model:

- Standing memory:
  - project instructions and stable user/project notes;
  - loaded and frozen at session start;
  - compact enough to live in the stable prefix.
- Saved memory index:
  - typed durable records with pointers or summaries;
  - included as a compact index, not full bodies;
  - full detail is read on demand through tools or recall.
- Dynamic recall:
  - task-relevant memory snippets;
  - inserted into the turn tail or dynamic context zone;
  - never mutates the stable prefix during an active session.

Work:

- Add a user-facing memory explanation command or doctor section that answers:
  - when memory is saved;
  - whether save is automatic or review-gated;
  - where memory files live;
  - which memory enters the prefix;
  - which memory is selected dynamically;
  - how to inspect, accept, reject, or repair memory proposals.
- Make mid-session memory saves behave like Reasonix's pending memory note:
  useful immediately through turn-tail context, but stable-prefix eligible only
  next session.
- Keep quality gates stronger than Reasonix:
  - evidence validation;
  - scope validation;
  - deduplication;
  - stale/conflict checks;
  - sensitive-data filtering;
  - review policy.
- If quick-add is added, prefer explicit `/remember <note>` first. A bare
  `# <note>` shortcut is useful but should be introduced only if it will not
  conflict with common issue-number or Markdown usage.
- Update the memory product docs after behavior changes.

Acceptance:

- The memory UX can be explained in one screen.
- Tests prove mid-session memory changes do not change the stable prefix.
- Memory proposal review stays enabled by default unless a narrower project
  policy explicitly allows auto-save.
- Run:

```bash
cargo test -q memory
cargo test -q prompt_context
cargo test -q request_preparation_controller
cargo check -q
```

## 11. Phase 5: Permission And Checkpoint Core Hardening

Goal: keep safety behavior strong while making the core easier to test.

Reasonix reference:

- `internal/permission/permission.go`
- `internal/checkpoint/checkpoint.go`

Priority Agent target:

- `src/permissions/mod.rs`
- `src/permissions/tests.rs`
- `src/engine/human_review.rs`
- `src/engine/checkpoint.rs`
- `src/tools/file_tool/history.rs`
- `src/tools/file_tool/mod.rs`
- `src/tools/rewind_tool`

Work:

- Keep permission policy as pure decision logic where possible:
  - deny beats ask;
  - ask beats allow;
  - read-only operations are allowed only when truly read-only;
  - risky bash/file operations carry explicit reasons.
- Ensure UI approval is just an approver, not hidden policy.
- Ensure file writer tools have preview/checkpoint metadata before mutation.
- Keep bash side effects classified separately from previewable file writes.
- Add tests for common ambiguity cases:
  - read-only bash commands;
  - bash commands that write through redirection;
  - file writes outside workspace;
  - memory clear/save;
  - MCP/plugin writes;
  - checkpoint restore after failed edit.

Acceptance:

- Permission policy can be tested without TUI/desktop.
- Checkpoint creation and restore behavior is clear in both logs and UI events.
- No validation or permission gate is weakened for eval convenience.
- Run:

```bash
cargo test -q permissions
cargo test -q checkpoint
cargo test -q file_tool
cargo check -q
```

## 12. Phase 6: TUI/Desktop Real-Path Parity

Goal: make the real product paths boring and comparable.

Reasonix reference:

- one `control.Controller` behind terminal, desktop, and serve frontends.

Priority Agent target:

- `src/tui/app.rs`
- `src/tui/screens/`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src/app/components/StatusBar.tsx`
- `apps/desktop/src/app/components/TraceDrawer.tsx`

Work:

- Define an event parity matrix:
  - assistant text;
  - tool card;
  - approval;
  - permission denial;
  - runtime diagnostic;
  - provider slow warning;
  - timeout;
  - cache usage;
  - context compaction;
  - verified/partial/not-verified closeout.
- Ensure TUI and desktop consume equivalent runtime events for full turns.
- Keep frontend state reducers smaller by splitting pure event normalization from
  rendering and sample/demo data.
- `apps/desktop/src/app/runEventState.ts` has been split below 1500 lines by
  moving runtime diagnostic and tool timeline presentation helpers out of the
  reducer; keep it on the watch list when future desktop events are added.
- Treat `apps/desktop/src-tauri/src/lib.rs` as watch-only unless the controller
  boundary makes a clean split obvious.

Acceptance:

- TUI and desktop show the same provider/cache/permission facts for the same
  full-agent turn.
- Desktop lightweight lane is labeled separately.
- Run:

```bash
cargo check -q
cargo check --features experimental-api-server -q
```

If desktop UI behavior changes, also run the local desktop smoke path and record
the observed result in the implementation note.

## 13. Phase 7: Daily Baseline Stabilization

Goal: make daily validation an official product baseline, not an ad hoc run.

Priority Agent target:

- `scripts/run_live_eval.sh`
- `scripts/live_eval_report_parser.py`
- `docs/DAILY_GATE_TEST_REPORT_2026-06-03.md`
- `docs/UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md`
- `target/external-runs/`

Work:

- Define a small official daily baseline:
  - read and summarize;
  - edit a file and run a narrow test;
  - permission denial/approval;
  - memory recall;
  - prompt-cache diagnostics;
  - provider timeout or slow-tail handling;
  - desktop or TUI real full-turn path.
- Keep weak-model failures classified by owner:
  - agent-flow bug;
  - harness bug;
  - provider/model weakness;
  - environment issue;
  - expected not-verified or partial closeout.
- Do not change runtime policy to hide weak-model mistakes.
- Shrink scripts only after the baseline behavior is stable. Current script
  size shows `scripts/live_eval_report_parser.py` and `scripts/run_live_eval.sh`
  are the next script simplification candidates.

Acceptance:

- Daily baseline command is documented and reproducible.
- Reports include required commands, diff state, proof, closeout status, and
  failure owner.
- Run:

```bash
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/workflow-production-gates.sh
```

Run live eval only when the provider environment is configured and the user
expects a real provider spend.

## 14. Phase 8: Code Size Stewardship

Goal: keep the repo maintainable without chasing arbitrary 500-line targets.

Current policy:

- Normal production source should avoid exceeding 1500 lines.
- Files above 1200 lines are watch-list files.
- Test files and scripts can have exceptions, but large scripts should be split
  when behavior changes.
- Do not split a file only to satisfy a number if the split makes runtime
  contracts harder to follow.

Current watch list from the latest report:

- `scripts/live_eval_report_parser.py` at 3005 lines: priority script split.
- `scripts/run_live_eval.sh` at 1892 lines: script split candidate.
- `apps/desktop/src/app/runEventState.ts` at 1489 lines: frontend reducer
  watch item.
- `src/tools/agent_tool/mod.rs` at 1472 lines: watch.
- `src/tui/slash_handler/learning.rs` at 1472 lines: watch.
- `apps/desktop/src-tauri/src/lib.rs` at 1468 lines: watch.
- `src/engine/improvement.rs` at 1458 lines: watch.
- `src/engine/scenario_matrix.rs` at 1453 lines: watch.
- `src/engine/streaming.rs` at 1449 lines: watch.
- `src/tools/file_tool/mod.rs` at 1443 lines: watch.
- `src/tools/bash_tool/mod.rs` at 1440 lines: watch.
- `src/tui/slash_handler/agents.rs` at 1437 lines: watch.
- `src/engine/conversation_loop/permission_controller.rs` at 1414 lines: watch.
- `src/memory/eval.rs` at 1408 lines: watch.
- `src/engine/conversation_loop/tool_result_controller.rs` at 1407 lines:
  watch.

Acceptance:

- No normal production source file drifts over 1500 lines without a documented
  exception.
- Every phase that touches a watch-list file either shrinks it or explains why
  no split was appropriate in that slice.
- Run:

```bash
scripts/file-size-report.sh --threshold 1200 --top 25
```

## 15. Recommended Implementation Order

1. Do Phase 0 first. The boundary map prevents accidental frontend/runtime
   divergence.
2. Do Phase 2 next if cache behavior is the highest product concern. The tests
   will protect later controller and memory work.
3. Do Phase 1 after the cache contract is clear enough to avoid moving unstable
   behavior around.
4. Do Phase 3 to reduce tool prompt surface once controller boundaries are
   visible.
5. Do Phase 4 to make memory UX simple without weakening existing gates.
6. Do Phase 5 and Phase 6 as hardening work on the real product paths.
7. Do Phase 7 continuously as the daily proof lane.
8. Keep Phase 8 active opportunistically whenever a watch-list file is touched.

## 16. Risks And Mitigations

Risk: over-unifying the runtime breaks currently working paths.

Mitigation: start with a wrapper/controller boundary. Do not move loop internals
until the full-turn path is proven through tests.

Risk: reducing default tools makes the agent less capable.

Mitigation: keep advanced tools available by route and slash command. The goal
is smaller default schema, not deletion.

Risk: memory simplification loses important quality gates.

Mitigation: simplify product explanation, not persistence policy. Keep proposal
review, evidence validation, deduplication, stale/conflict checks, and sensitive
data filtering.

Risk: cache tests become brittle because providers report cache differently.

Mitigation: test stable-prefix fingerprints and request shapes locally. Treat
provider `cached_tokens` as an observed diagnostic, not the only source of truth.

Risk: daily live eval chases weak-model mistakes.

Mitigation: continue classifying `failure_owner`. Honest `partial`,
`not_verified`, or provider-owned failure is valid when runtime evidence is
correct.

## 17. Non-Goals

- Do not rewrite Priority Agent in Go or recreate Reasonix package-for-package.
- Do not remove evidence ledger, validation gates, permission checks, or
  checkpoints to simplify the loop.
- Do not force every file under 500 lines immediately.
- Do not add more always-on prompt rules for one-off model mistakes.
- Do not make desktop or TUI maintain separate agent-loop semantics.

## 18. Definition Of Done

This next phase is complete when:

- there is one documented full-agent command/event boundary used by TUI and
  desktop;
- stable-prefix tests fail when dynamic memory, task state, repair hints,
  provider notices, or background notes mutate the prefix;
- the default tool schema is smaller and route-scoped expansion is tested;
- memory can be explained as standing docs, saved index, and dynamic recall;
- mid-session memory updates do not bust the cache-stable prefix;
- permission and checkpoint cores remain testable without frontend UI;
- daily baseline reports are reproducible and classify failures honestly;
- no normal production source file exceeds 1500 lines without a documented
  exception.

The expected result is a product that keeps Priority Agent's stronger local
coding workflow while gaining Reasonix-style simplicity at the boundaries.

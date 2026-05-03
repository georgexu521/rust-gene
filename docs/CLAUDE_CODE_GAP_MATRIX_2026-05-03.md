# Claude Code Gap Matrix

Date: 2026-05-03

Scope: compare the current `rust-agent` checkout with the local Claude Code
source tree at `/Users/georgexu/Desktop/claude`. This is a product-maturity
gap matrix, not a raw feature-count checklist. The project has enough runtime
foundation to write code, edit files, run validation, and recover from some
failures, but it is not yet a Claude Code replacement.

## Current Verdict

`rust-agent` is now a working coding-agent runtime with an interactive CLI,
tool execution, memory, traces, permissions, workflow routing, required
validation, and live eval artifacts. It can perform controlled coding tasks
end-to-end.

The remaining gap is product depth:

- Claude Code has mature per-tool semantics, rich terminal UI, subagent
  isolation, hooks, remote/session control, and polished recovery loops.
- `rust-agent` has many of the same surface areas, but several are still
  entry-level, partially wired, or under-tested in broad real-world tasks.

Approximate maturity:

| Area | Current level |
|------|---------------|
| Controlled repo coding tasks | 65-75% of Claude Code-like behavior |
| Daily dependable replacement | 55-60% |
| Product maturity | 40-50% |
| Experimental planning/memory ideas | Stronger than raw maturity suggests, but not fully productized |

## Evidence Reviewed

Current project evidence:

- `docs/PROJECT_STATUS.md`: current runtime spine and remaining product work.
- `docs/benchmarks/live-eval-20260501-testing-plan.md`: live eval history,
  known failure modes, and repair-loop behavior.
- `docs/benchmarks/live-live-eval-20260503-152320/code-change-verification-repair-loop/report.md`:
  latest real agent smoke for the code-change verification repair loop.
- `src/engine/conversation_loop/mod.rs`: current workflow, validation, closeout,
  tool execution, and recovery path.
- `src/tools/`: broad registered tool surface.
- `src/tui/app.rs`: slash-command and interactive CLI surface.

Local Claude Code evidence:

- `/Users/georgexu/Desktop/claude/src/query.ts`: mature streaming loop,
  tool orchestration, abort/retry behavior, compaction, model fallback, and
  tool result handling.
- `/Users/georgexu/Desktop/claude/src/tools/`: high-depth per-tool product
  semantics and UI helpers.
- `/Users/georgexu/Desktop/claude/src/components/`: rich terminal UI
  components for diffs, approvals, history, status, MCP, and context views.
- `/Users/georgexu/Desktop/claude/src/types/hooks.ts` and hook utilities:
  typed lifecycle events and hook execution.
- `/Users/georgexu/Desktop/claude/src/bridge/` and
  `/Users/georgexu/Desktop/claude/src/remote/`: remote session and desktop
  handoff product surface.

## Gap Matrix

| Dimension | Current `rust-agent` | Claude Code reference shape | Gap | Priority |
|-----------|----------------------|-----------------------------|-----|----------|
| Coding loop | Can inspect, edit, run required validation, and repair narrow failures. Recent local baseline reached `1006 passed`. | First-pass implementation, streaming tool use, retry, compaction, and closeout are mature daily paths. | Reduce first-pass hesitation, validation fragility, and broad-task repair misses. | P0 |
| Tool semantics | Broad registry across file, bash, grep, web, memory, MCP, git, LSP, worktree, agent, and utility tools. | Each tool has deeper prompt guidance, UI display, safety classification, summaries, and recovery behavior. | Move from surface parity to per-tool depth. | P0 |
| Validation/closeout | Required validation is now explicit and long-running commands emit progress, but this path was recently fixed. | Validation and final reporting are stable parts of the product loop. | Keep hardening stale-evidence detection, env-prefixed commands, and closeout normalization. | P0 |
| CLI/TUI UX | Interactive CLI and many slash commands exist. Some panels are trace-backed. | Rich Ink components for diff review, context visualization, MCP approval, history search, status, remote sessions, and settings. | Make high-use panels production-grade and mark placeholders honestly. | P1 |
| Subagents | Agent/task/swarm infrastructure exists and can hand off work in constrained cases. | Subagents have profiles, fork/resume semantics, memory snapshots, tool scopes, status UI, and lifecycle hooks. | Add reliable forked context, per-agent permissions, resumable state, and visible progress. | P1 |
| Hooks | Pre/post tool and related hook concepts exist in parts of the runtime. | Typed lifecycle covers prompt, tool, permission, notification, subagent, file changes, compaction, and more. | Build one typed hook runtime and route all major lifecycle events through it. | P1 |
| Memory/context | Memory namespace search, conflict hints, planning feedback, context compression, and traces exist. | Memory extraction and compaction are background/productized, with smoother long-session behavior. | Make memory extraction, compaction carryover, and retrieval provenance consistently visible and low-friction. | P1 |
| MCP/integrations | MCP tools/resources/health are present; API/WebSocket/SSE and Telegram framework exist. | MCP prompts/commands, auth repair, approval UX, and remote session flows are more complete. | Productize MCP auth/repair/prompt UX and bridge/session operations. | P1 |
| Permissions | Rule-based permissions, confirmations, and some policy surfaces exist. | Permission decisions are deeply integrated with UI, tool semantics, hooks, and retry after denial. | Add richer review flows, import/export confidence, and command-specific explanations. | P1 |
| Product delivery | CLI/TUI plus experimental API/platform paths. | CLI, desktop/remote bridge, IDE surfaces, and user-facing integration flows are mature. | Package, onboarding, updater, remote workflow, and integration polish. | P2 |
| Evaluation | Live evals and regression plans exist; controlled scenarios have improved. | Mature product behavior is proven across many daily tasks and edge cases. | Build a stable 20-30 task replay matrix with pass/fail trend tracking against Claude/Codex baselines. | P0 |

## Highest-Value Implementation Order

### P0: Make The Coding Loop Decisive

Goal: make broad code-change tasks finish with fewer repair turns and clearer
evidence.

Deliverables:

- [x] Add an implementation-intent checkpoint before the first edit on code-change
  tasks: target files, intended behavior change, validation commands, and risk.
- [x] Track first-pass edit success, repair count, validation count, and final
  evidence quality in `TurnTrace`.
- [x] Treat env-prefixed validation commands such as
  `env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test ...` as first-class
  validation commands.
- [x] Keep required validation evidence separate from opportunistic auto-tests.
- [x] Add regression tasks for broad edits, stale validation evidence, and empty
  closeout output.

Acceptance:

- A real code-change eval produces an implementation-intent trace before edit.
- Required validation success cannot be satisfied by stale previous commands.
- Final answer includes the exact validation evidence or a clear explanation
  of why validation was not run.

### P0: Upgrade Tool Semantics For Core Coding Tools

Goal: file edit, bash, grep, git, and project search should behave like
product-grade coding tools, not generic JSON executors.

Deliverables:

- [x] Add per-tool execution summaries suitable for final closeout and trace views.
- [x] Add stale-read detection for file edits when the file changed after read.
- [x] Expand bash command classification around destructive commands, validation
  commands, long-running commands, and environment-prefixed commands.
- [x] Improve grep/project search no-result recovery so the agent changes search
  strategy instead of looping.
- [x] Add tests that assert tool summaries and recovery metadata, not only success
  booleans.

Acceptance:

- Tool failure reports include actionable recovery metadata.
- File edits refuse or warn on stale read state.
- Bash validation classification handles cargo, npm, pytest, shell wrappers,
  and env-prefixed commands.

### P1: Productize Subagents

Goal: subagents should be dependable enough for real parallel code work.

Deliverables:

- [x] Define explicit `AgentProfile` records: role, allowed tools, context mode,
  timeout, output contract, and risk policy.
- [x] Support forked context, inherited context, and minimal-context modes.
- [x] Persist subagent status and result artifacts in the session store.
- [x] Show subagent progress in CLI status/trace views.
- [x] Add tests for agent timeout, failure, cancellation, and resumable result
  retrieval.

Acceptance:

- A parent task can spawn a verifier or worker and receive a structured result.
- Subagent tool permissions are enforced independently from the parent.
- `/trace last` shows subagent lifecycle events.

### P1: Build One Typed Hook Runtime

Goal: hooks should become a real lifecycle API instead of scattered callbacks.

Deliverables:

- [x] Define typed events for prompt submit, pre/post tool, permission request,
  validation start/end, subagent start/end, file change, compaction, and session
  end.
- [x] Route all hook execution through one manager with timeout, redaction,
  blocking/non-blocking modes, and structured results.
- [x] Expose hook status through CLI and trace.

Acceptance:

- A pre-tool hook can block a risky command.
- A post-tool hook can add structured trace metadata.
- Hook failures are visible and do not crash the main loop.

### P1: CLI Product Polish

Goal: make the common coding workflow visible and pleasant enough for daily use.

Deliverables:

- Prioritize diff review, approval panels, status line, history search,
  context visualization, and tool expansion views.
- [x] Categorize all slash commands as `production`, `usable`, or `placeholder`.
- [x] Hide or clearly label placeholders in help/model-visible surfaces.
- [x] Add rendered command-palette smoke tests for placeholder labels, usable
  labels, and contextual permission actions.
- [x] Add rendered approval-panel smoke tests for bash risk review and
  file-write scope/preview.
- [x] Add rendered statusline and tool-output viewer smoke tests for active
  tool state and output controls.
- [x] Add rendered diff viewer smoke tests for unified diff and empty states.

Acceptance:

- `/help` does not imply placeholder commands are production features.
- A code-change turn shows concise progress, tool results, validation, and
  final evidence without digging through raw logs.

### P0: Evaluation Replay Matrix

Status: initial matrix landed.

- `evalsets/coding_replay_matrix.yaml` now covers 20 deterministic coding
  replay scenarios.
- `/eval json <name|all>` emits machine-readable pass/fail reports, and
  `/eval record <name|all>` writes timestamped JSON reports under
  `target/eval-reports/` for trend collection.
- `/eval trend [limit]` summarizes recent persisted reports and shows pass/fail
  deltas against the previous run.
- Persisted eval report JSON is backward compatible and can carry optional
  external baseline metadata for Claude/Codex-style comparisons.
- Git tool execution now honors `ToolContext.working_dir` and returns
  structured summary/recovery metadata for closeout and trace consumers.
- Final closeout includes structured evidence counts, and bash progress labels
  now use validation command classification for cargo test/check/clippy and
  similar long-running validation commands.
- Current full local baseline after this batch: `1053 passed; 0 failed`.
- Latest opt-in live smoke after this batch:
  `live-eval-20260503-152320 code-change-verification-repair-loop`, status
  `ok`, with `verification_passed=true`, `stage_validation_passed=true`, and
  `closeout_status=passed`.

### P2: Product And Ecosystem Completion

Goal: close the remaining product distribution gap after the core coding loop
is stable.

Deliverables:

- MCP server mode and polished MCP auth/repair.
- Plugin package lifecycle: install, validate, trust, update, remove.
- Remote/session bridge hardening and replay.
- IDE/desktop onboarding and packaging.
- Optional voice/web/desktop surfaces only after CLI reliability is strong.

## What Not To Do Next

- Do not count tools or commands as parity just because entries exist.
- Do not add more slash commands until current commands have maturity labels
  and smoke tests.
- Do not optimize exotic integrations before the P0 coding loop metrics are
  stable.
- Do not claim Claude Code parity without replay evidence on broad real tasks.

# Project Status

Last updated: 2026-05-05

## Summary

Priority Agent is now an interactive coding CLI with a stateful runtime spine:
intent routing, turn traces, session goals, memory, permissions, recovery plans,
MCP health, CLI observability panels, and required validation closeout.

The recent closure plan is complete:

| Area | Status | Commit |
|------|--------|--------|
| Tool failure recovery and tool outcome learning | Complete | `fd74714` |
| Learning-driven tool selection | Complete | `44b4250` |
| Goal drift visibility | Complete | `bd12f64` |
| Memory namespace search and conflict hints | Complete | `934f7fe` |
| MCP health-aware visibility and resource traces | Complete | `f0f4a95` |

Latest verified baseline observed after the 2026-05-05 CLI scrollback progress
batch:

```text
1060 passed; 0 failed
```

Verified with:

```bash
cargo check --quiet
cargo test --quiet -- --test-threads=1
cargo clippy --all-features -- -D warnings
env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1
```

Latest live coding workflow smoke:

```text
live-eval-20260503-152320 code-change-verification-repair-loop: ok
verification_passed=true stage_validation_passed=true closeout_status=passed
```

## Completed Runtime Spine

- `TurnTrace` records prompt, routing, memory, context, tool, permission,
  recovery, goal drift, assistant, and MCP resource events.
- `IntentRouter` chooses workflow/retrieval/reasoning policy and now consumes
  learning events from tool outcomes.
- `SessionGoal` tracks the active goal; high drift requires approval and
  medium drift is visible in `/goal drift` and `/quick`.
- Tool failures attach recovery metadata and persist `tool_outcome` learning
  events.
- Code-change turns record implementation intent before edits, and env-prefixed
  validation commands are classified as validation evidence.
- Final closeout now includes an evidence summary with changed-file,
  validation-status, and acceptance-status counts.
- Core coding tools now attach structured execution summaries; file edits refuse
  stale-read writes by default; bash command classification covers shell/env
  wrappers and common validation families; git tool execution now honors the
  tool working directory and returns structured summary/recovery metadata.
- Memory search spans project, user, topic, and agent namespaces with simple
  conflict detection.
- MCP status and tool/resource visibility are health-aware and approval-aware.
- Hook execution uses typed lifecycle events, records structured run results,
  and is visible through TurnTrace and `/hooks`.
- Tool execution progress labels are classifier-aware for bash validation
  commands, so cargo test/check/clippy and similar commands show specific
  progress instead of generic shell execution text.
- Subagents have explicit profile contracts, independent tool scopes, lifecycle
  trace events, durable result artifacts, and manager tests for timeout, failure,
  cancellation, and resumable results.
- Slash commands are labeled as `production`, `usable`, or `placeholder` in help
  and command-palette surfaces, with rendered command-palette smoke tests for
  placeholder, usable, and contextual permission actions plus approval-panel
  smoke tests for bash and file-write review flows, statusline active-tool
  state, tool-output viewer controls, and diff viewer output/empty states.
- Evalsets include a 25-scenario deterministic coding replay matrix, JSON
  report output, and `/eval record <name|all>` persisted report files for
  pass/fail trend collection; `/eval trend [limit]` summarizes recent persisted
  reports, deltas against the previous run, and optional external baseline
  metadata when present.
- The layered workflow gates now cover focused, standard, full-local, and
  opt-in live-smoke validation; the latest live smoke exercised the real
  code-change repair path and passed with full-suite validation.
- CLI panels are increasingly backed by actual runtime state, not decoration.
- `karpathy-guidelines` is bundled as a coding behavior skill and exposed
  through `/skills`, `/karpathy <task>`, and code-change reflection checks.
- Repeated successful workflows can now become reviewed skill candidates through
  `/skill-proposals`; accepted candidates are untrusted until explicitly
  applied into the user skill path.
- Learning and high-confidence retrieved memory now feed back into workflow
  planning weights with traceable before/after audit records.

## Product Surface

Primary interface:

- `priority-agent`
- `priority-agent --cli`

Compatibility:

- `priority-agent --tui` starts the compatibility full-screen terminal interface.

Secondary interfaces:

- HTTP API with REST/WebSocket/SSE behind `experimental-api-server`.
- Platform adapter framework with Telegram implemented.
- MCP client over stdio, WebSocket, and HTTP.

## Documentation Status

Canonical current docs:

- `README.md`
- `docs/PROJECT_STATUS.md`
- `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`
- `docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`
- `docs/REMAINING_CLOSURE_PLAN.md`
- `AGENTS.md`

Historical docs kept for reference:

- `PLAN.md`
- `CAPABILITY_MATRIX.md`
- `docs/CLAUDE_GAP_SCORECARD.md`
- `docs/workflow/*`

Removed as obsolete:

- `FEATURE_COMPARISON_CLAUDE_CODE.md`
- `FEATURE_COMPLETENESS_REPORT.md`

Both removed reports described an early state with very few tools, no memory,
and MCP as a stub. That no longer matches the codebase and was more misleading
than useful.

## Remaining Work

The latest 5-item closure plan is complete, and the first Claude-gap P0/P1
implementation batch is now landed. The remaining work is now product maturity,
not missing foundations:

1. Continue measuring broad code-change first-pass success and repair count
   against the replay matrix and live eval tasks.
2. Continue hardening long-running command progress around cancellation,
   timeout, and streamed partial output.
3. Expand rendered command-level smoke tests beyond core panels into broader
   settings and history surfaces.
4. Populate persisted eval reports with real external Claude/Codex baseline
   data once those baseline runs are available.
5. Continue CLI polish based on trace-backed state: command palette, statusline,
   approval panels, tool expansion, and settings visibility.
6. Harden ecosystem integrations: MCP server mode, plugins, remote workflows,
   Discord/Slack adapters if they become product priorities.
7. Keep docs synchronized with tests and current behavior.

Latest maintenance note:

- `cargo clippy --all-features -- -D warnings` is clean as of 2026-05-05.
- `scripts/validate_docs.sh` counted 74 registered tool entries and 130 command
  constants, then passed all required docs, all-features build, and the
  workflow-enabled full test suite.
- `/resume` now resolves recent conversations by number, id prefix, title/model
  keyword, or message search, and restored sessions show a recent context
  preview.
- The scrollback-first interactive shell now prints concise long-running tool
  progress lines, so validation work is visible without switching to a
  full-screen interface.
- Live eval task parsing no longer depends on PyYAML for prepare/collect paths,
  and the dashboard-summary seeded fixture now preserves the summary entrypoint
  while stubbing only `summary_task()`.
- A fresh dashboard-summary agent-run on 2026-05-05 confirmed the harness-side
  PyYAML traceback is gone; the remaining failure is a clean `llm_reasoning`
  signal because the agent produced no code diff and left the seeded
  `summary_task()` stub failing.
- Live eval summaries now include pass/failure rates, real code-change pass
  counts, plan-only pass counts, seeded no-diff failure counts, and aggregated
  failure modes. `scripts/live-eval-summary-smoke.sh` covers this without
  running an LLM and is part of the quick coding workflow gate.
- `scripts/live-eval-aggregate-summary.sh` now refreshes existing benchmark
  summaries and writes `docs/benchmarks/live-eval-shortfall-summary.md`; the
  current aggregate scans 111 runs / 110 task reports and shows the largest
  shortfalls are verification/closeout failure, required-command failure, and
  older report metadata gaps. The aggregate also backfills conservative
  inferred owners for old reports while keeping `owner_metadata_missing`
  separate, so historical format gaps do not hide product failure patterns.
- Live eval reports now classify action-checkpoint stops separately
  (`action_checkpoint_no_patch`, `action_checkpoint_invalid_tools`,
  `patch_synthesis_no_change`) and the aggregate report has an `Agent Flow
  Stops` section. This separates model reasoning failures from execution-loop
  failures where the agent never produced an applicable patch.
- Focused repair prompts now consistently allow exactly one targeted
  `file_read`/`grep` lookup before patching instead of contradicting that with a
  blanket read/search ban. Action-checkpoint unexposed-tool errors now list the
  currently exposed tools and the expected repair path.
- Action checkpoint now enforces that targeted lookup budget in the exposed
  tool set: after one successful `file_read`/`grep` lookup, the next focused
  repair request hides read/search tools and forces patch tools only.
- A live A/B on `memory-save-quality-gate` confirmed the lookup-budget change
  moved the run from `agent_flow/action_checkpoint_no_patch` with zero edits to
  a real repair loop with changed files, validation, guided debugging,
  acceptance review, and final `llm_reasoning` failure. The remaining failure is
  product reasoning/repair quality, not checkpoint tool flow.
- Verification and acceptance failures now generate a deterministic
  `RepairSpec` prompt that lists failed commands, extracted failing tests,
  required next-patch constraints, forbidden fixes, and validation commands.
  This gives the model a structured repair target without writing the product
  patch for it.

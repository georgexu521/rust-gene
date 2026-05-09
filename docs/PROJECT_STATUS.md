# Project Status

Last updated: 2026-05-09

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

Latest deterministic test baseline observed after the 2026-05-09 Batch 4
`patch_recovery` extraction:

```text
1140 passed; 0 failed
```

Validated in this pass with:

```bash
cargo test -q
```

Latest recovery commits and planning artifacts:

| Area | Commit |
|------|--------|
| Route live coding evals as code changes | `e18de91` |
| Harden live eval patch recovery | `b2ff20c` |
| Record live eval recovery evidence | `6df0039` |
| Add next development plan | `467c3b0` |
| Add bash exposure diagnostics | `d025d6a` |
| Guard terminal and filesystem grounding | `2b1852e` |
| Keep grep evidence raw for patching | `3344363` |
| Extract patch recovery module | this change |
| Extract validation runner helpers | this change |
| Extract repair controller helpers | this change |
| Extract closeout controller helpers | this change |

The all-features clippy and experimental API checks were last recorded as
passing in the post-recovery baseline before this docs-only reset. Rerun them
before a release cut or behavior change merge.

Latest live coding workflow smoke:

```text
checkpoint-function-anchor-20260509-120047 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
```

Latest Batch 3 live-suite run:

```text
capability-now-20260509-144729 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
rerun_after=3344363 grep raw evidence fix
capability-now-20260509-143251 live-eval-dashboard-summary: failed
diff=no required_command_status=failed closeout_status=not_verified failure_owner=agent_flow
root_cause=grep markdown highlighting polluted patch anchors
capability-now-20260509-142349 memory-save-quality-gate: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
capability-now-20260509-141759 frontend-book-notes-localstorage: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
capability-now-20260509-140733 backend-todo-api-crud: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
warnings=tool_errors_seen,earlier_verification_failed_before_repair
capability-now-20260509-135556 code-change-verification-repair-loop: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
```

Latest aggregate live-eval snapshot:

```text
generated=2026-05-09 14:58:04 +0800
runs_scanned=142 task_reports=142 pass_rate=40/142
instrumented_slice=18/50 passed
real_code_change_passes=13 seeded_no_diff_failures=17
```

Read this aggregate as historical plus current evidence. It still includes many
older reports from before structured `failure_owner`, `eval_intent`, and
adaptive-trigger metadata, while the newest dashboard-summary recovery is a
current passing run with a real code diff.

## Completed Runtime Spine

- `TurnTrace` records prompt, routing, memory, context, tool, permission,
  recovery, goal drift, assistant, and MCP resource events.
- Maintainability cleanup is underway: focused action-checkpoint helpers now
  live outside the core conversation loop, deterministic patch repair is routed
  through a named rule registry with owner/review metadata, live-eval report
  parsing is shared by summary and aggregate scripts, and `/permissions` has
  its own slash-handler module.
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
- Root `AGENTS.md` is now a compact runtime guide with historical material
  archived under `docs/archive/`; instruction loading prefers the
  `## Agent Runtime Guidance` section instead of prefix-truncating long project
  notes.
- Runtime diet gates now cover representative prompt samples for direct answer,
  scoped file deletion, Python code creation, running-issue debugging, and
  Claude/opencode instruction-design comparison. Tool-surface tests enforce
  route-level caps and prompt-context tests enforce a common-turn token budget.
- Core tool contracts now carry common usage boundaries directly: `file_edit`
  rejects `file_read` line-prefix copy/paste in edit anchors, `file_write`
  reports full-file replacement guidance when overwriting, `bash` is scoped to
  shell/validation work, `agent` discourages blocking delegation, and
  `skill_view` fences skill text as guidance rather than user instruction.
- Built-in subagent profiles now have role-scoped default tool surfaces:
  explorer/planner/verifier stay read-only or validation-only, implementer gets
  edit/write/validation tools, and no built-in profile exposes recursive
  `agent` or `swarm` by default.
- Memory and skill context are now fenced as background guidance. Light/Web/None
  routes do not receive stale memory context, skill listing is compact
  discovery-only, and runtime diet traces report memory, retrieval, and skill
  summary budgets.
- User-facing closeout now defaults to concise assistant text for ordinary
  passed or not-verified low/medium-risk code changes, while high-risk,
  failed, partial, explicit debug/full, and live-eval closeouts retain the full
  structured `Closeout:` block.
- Terminal and filesystem truth guards now catch two high-trust UX failures:
  claiming bash is unavailable when it is exposed, and answering current local
  filesystem state without first using exposed read/list tools. The correction
  stays runtime-owned instead of adding longer always-on prompt rules.
- `glob` now treats `**/` as zero-or-more directories for agent-facing patterns
  and sorts shallow paths first before truncation, so broad local inspection is
  less likely to hide top-level entry files.
- `grep` now leaves visible match lines as raw source and carries match text in
  structured metadata, preventing Markdown emphasis from contaminating
  file-edit anchors and patch synthesis.

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
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/NEXT_DEVELOPMENT_PLAN_2026-05-09.md`
- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`
- `AGENTS.md`

Historical docs kept for reference:

- `PLAN.md`
- `CAPABILITY_MATRIX.md`
- `docs/archive/AGENTS_PROJECT_GUIDE_PRE_RUNTIME_DIET_2026-05-08.md`
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
implementation batch is now landed. The current plan is
`docs/NEXT_DEVELOPMENT_PLAN_2026-05-09.md`: treat Priority Agent as a reliable
LLM execution environment, move hard constraints into runtime/tool contracts,
and measure progress with current live-eval evidence instead of prompt length.
The remaining work is now product maturity, not missing foundations:

1. Continue measuring broad code-change first-pass success and repair count
   against the replay matrix and live eval tasks.
2. Execute the next plan in order: Batch 1 baseline hygiene, Batch 2
   terminal/filesystem truth, Batch 3 five-case live suite, and Batch 4
   `patch_recovery` / `validation_runner` / `repair_controller` /
   `closeout_controller` extraction are now landed; next is Batch 4
   `tool_orchestrator` extraction.
3. Continue hardening long-running command progress around cancellation,
   timeout, and streamed partial output.
4. Expand rendered command-level smoke tests beyond core panels into broader
   settings and history surfaces.
5. Populate persisted eval reports with real external Claude/Codex baseline
   data once those baseline runs are available.
6. Continue CLI polish based on trace-backed state: command palette, statusline,
   approval panels, tool expansion, and settings visibility.
7. Harden ecosystem integrations: MCP server mode, plugins, remote workflows,
   Discord/Slack adapters if they become product priorities.
8. Keep docs synchronized with tests and current behavior.

Latest maintenance note:

- `cargo test -q` is clean as of 2026-05-09 with `1140 passed; 0 failed`.
- `conversation_loop/mod.rs` is down to 6958 lines after moving patch synthesis,
  deterministic patch recovery, synthesized patch validation, required
  validation commands, validation command classification, verification source
  context, guided validation debugging, acceptance repair review, and final
  closeout appending into dedicated conversation-loop modules.
- `cargo clippy --all-features -- -D warnings` was last recorded clean in the
  post-recovery baseline before this docs-only reset.
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
- The latest dashboard-summary agent-run,
  `checkpoint-function-anchor-20260509-120047`, produced a real diff, passed
  required commands, passed verification/stage validation, and ended with
  `failure_owner=none`.
- Live eval summaries now include pass/failure rates, real code-change pass
  counts, plan-only pass counts, seeded no-diff failure counts, and aggregated
  failure modes. `scripts/live-eval-summary-smoke.sh` covers this without
  running an LLM and is part of the quick coding workflow gate.
- `scripts/live-eval-aggregate-summary.sh` now reads benchmark `report.md` and
  quality artifacts directly instead of overwriting per-run `summary.md` files,
  then writes `docs/benchmarks/live-eval-shortfall-summary.md`; the current
  aggregate scans 136 task reports, with 35 passed and 101 failed. The cleaner
  instrumented slice has 44 reports, 13 passed, 31 failed, and shows
  `agent_flow` at 43.2%, `llm_reasoning` at 20.5%, and eval-harness failures at
  6.8%.
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
- Initial no-diff action checkpoints now immediately attempt deterministic
  patch fallback when a safe hand-written rule matches the gathered evidence.
  Generic LLM patch synthesis remains opt-in, but known repair cases no longer
  need an extra model turn before the fallback can apply.
- Code-change workflow strictness is now adaptive instead of medium-risk by
  default: required validation, first code change, failed verification,
  acceptance rejection, and repeated no-edit progress activate the heavier
  judgment/validation/repair path automatically. Closeout evidence records the
  trigger labels so benchmark reports can explain why strict mode engaged.
- Adaptive workflow triggers are first-class trace events and live-eval
  summaries now expose a `triggers` column plus aggregate trigger distribution,
  so strict-mode activation can be measured without parsing fallback prose.

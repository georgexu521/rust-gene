# Project Status

Last updated: 2026-05-17

## Summary

Priority Agent is now an interactive coding CLI with a stateful runtime spine:
intent routing, turn traces, session goals, memory, permissions, recovery plans,
MCP health, CLI observability panels, and required validation closeout.

Current stage:

- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md` is complete through its
  follow-up implementation phases. Future runtime-diet work should come from
  live-use gaps, release-hardening gates, or a newly reviewed plan.
- `docs/NEXT_AGENT_CORE_CODING_QUALITY_PLAN_2026-05-11.md` is complete for its
  current Phase 1-4 scope: main-loop splitting reached the first line-count
  target, terminal and file-quality contracts are in place, and the real
  `core-coding-quality` rerun is current.
- `docs/CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md` remains the
  ownership map for future loop work, but the next step is no longer more
  low-level `run_inner` extraction by default.
- `docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md` remains the
  reference map for product maturity work against Claude Code and opencode.
- Current implementation focus has moved to product maturity: broader
  trace-backed evaluation, behavior-level memory/skill assertions, long-running
  terminal UX, CLI polish, and external baseline data.
- The next product-maturity slice is now the real-project coding gauntlet:
  `docs/REAL_PROJECT_CODING_GAUNTLET_PLAN_2026-05-17.md`. The live-eval runner
  supports `--case real-project-coding`, and live summaries now include a
  coding-gauntlet evidence section for agent-run tasks without changing the
  existing task matrix format. The first 15-task gauntlet run exposed two
  memory/product-maturity failures, and both have passing targeted reruns with
  behavior assertions and required validation now green. The first generic
  repair-planner slice now appends source snippets from failed required
  validation output, with targeted backend and frontend repair reruns passing.
- The first provider-protocol regression matrix slice is complete:
  OpenAI-compatible, MiniMax, and Kimi request conversion now share provider
  message normalization; pure assistant `tool_calls` omit empty content,
  orphan/aborted tool results are dropped before provider requests, MiniMax
  keeps system-message merging, and provider 400s that mention tool-result
  ordering are classified as `provider_protocol` instead of generic unknown
  failures.

The recent closure plan is complete:

| Area | Status | Commit |
|------|--------|--------|
| Tool failure recovery and tool outcome learning | Complete | `fd74714` |
| Learning-driven tool selection | Complete | `44b4250` |
| Goal drift visibility | Complete | `bd12f64` |
| Memory namespace search and conflict hints | Complete | `934f7fe` |
| MCP health-aware visibility and resource traces | Complete | `f0f4a95` |

Latest deterministic local test baseline observed during the 2026-05-16 and
2026-05-17 core-coding-quality closure, file-patch evidence integration,
file-patch encoded bytes history accuracy, terminal-task `/status` visibility,
terminal-task summary metadata,
foreground/PTY terminal-task data, background-shell terminal-task data,
route-scoped tool-test move, file-edit LSP document sync/diagnostics summary,
file-edit diff metadata, file-mutation lock, file text codec,
file-state tracker, partial-read edit state, file-read/search evidence
metadata, foreground PTY smoke, interactive-shell PTY diagnostic,
background-shell handles/output artifacts/task listing, shell-result
duration/schema/artifacts, shell-command UI summary, shell-command category
permission risk, shell-command category classifier, terminal provider-schema
exposure diagnostic, explicit patch-synthesis fallback boundary,
focused-repair proposal boundary, provider-protocol matrix,
permission-controller, context-budget, tool-result-budget, schema-gate, and
tool-result normalizer work, the file-patch write-mode guard,
live-eval unscored-report classification fix, deterministic patch-rule
priority, required-validation acceptance closeout fallback, and required-command
closeout evidence for no-diff audit tasks, plus Phase 4 ToolExecutionRecord
evidence-ledger persistence slices, including durable route/resource-policy and
execution-mode context:

```text
1453 passed; 0 failed
```

Latest live product baseline:

```text
core-quality-real-rerun-20260517-091952: 8/8 passed
failure_owner=none for every case
required_command_status=ok for every case
real code-change pass=3, audit/no-diff pass=5
```

Latest real-project coding gauntlet checkpoint:

```text
latest full rerun after terminal closeout evidence:
real-project-coding-20260517-192347: 15/15 passed
behavior_assertions=3, behavior_assertions_passed=3
required-validation passes=15/15
coding-gauntlet likely clean passes=7, repaired passes=4
real code-change passes=10, audit/no-diff passes=5
failure_owner=none for every case
closeout_status=passed for every case
backend-todo-api-crud: status=ok, failure_owner=none
frontend-book-notes-localstorage: status=ok, failure_owner=none
core-terminal-install-run: status=ok, failure_owner=none
core-provider-roundtrip: status=ok, failure_owner=none
memory-save-quality-gate: status=ok, failure_owner=none
persistent-memory-planning-context: status=ok, failure_owner=none
skill-promotion-gate: status=ok, failure_owner=none
warnings observed but non-failing:
- audit/no-diff warnings on audit/regression-check tasks
- tool_errors_seen in repaired/probe tasks with passing required commands
targeted Phase 3 repair reruns after required-validation source context:
- repair-planner-frontend-20260517-181652: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed
- repair-planner-backend-20260517-182004: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed,
  warnings=earlier_verification_failed_before_repair,
  earlier_stage_validation_failed_before_repair
targeted closeout-evidence rerun:
- terminal-closeout-20260517-191432: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed,
  runtime validation=passed:2/2
previous full rerun before terminal closeout evidence:
real-project-coding-20260517-183221: 14/15 passed

previous post-repair full rerun:
real-project-coding-20260517-171819: 12/15 passed

first gauntlet baseline before targeted repair:
real-project-coding-20260517-153331: 13/15 passed
coding-gauntlet evidence: likely clean passes=7, repaired passes=2,
required-validation passes=13/15, first-write observed=10/15
failures:
- memory-save-quality-gate: failure_owner=llm_reasoning
- persistent-memory-planning-context: failure_owner=agent_flow
targeted repair reruns:
- memory-save-rerun-20260517-170500: status=ok, failure_owner=none,
  behavior_assertions=passed, required_command_status=ok
- persistent-memory-rerun-20260517-172000: status=ok, failure_owner=none,
  behavior_assertions=passed, required_command_status=ok
note=persistent-memory-rerun-20260517-171000 exposed the intermediate
build_memory_context borrow repair path before the final passing rerun
```

Latest memory/skill product-maturity behavior baseline:

```text
product-maturity-seeded-fixes-20260517-143047: 3/3 passed
behavior_assertions=3, behavior_assertions_passed=3
status_counts=passed=3
failure_owner=none for memory-save-quality-gate, skill-promotion-gate, and
persistent-memory-planning-context
real code-change pass=3
deterministic patch repair rules now run before model patch synthesis when a
high-confidence rule matches current evidence
required validation acceptance can close out deterministically even when
workflow judgment was skipped after a non-JSON response
```

Current terminal slice: `bash mode=background` returns a shell handle,
`bash_output` reads bounded output from that handle, `bash_cancel` stops the
process group, foreground timeout results now carry structured
`shell_result.timed_out=true`, and CLI/TUI tool views have explicit
backgrounded/timed-out/cancelled states. Long background output now also writes
an `output_path` artifact under `.priority-agent/tool-results/<session>/...`.
`bash_tasks` lists known background shell handles when the model needs to
recover or inspect active tasks. Foreground bash, `bash mode=pty`, and
background-shell outputs now expose `terminal_task` structured facts with task
id, status, timestamps, duration, artifact path, terminal kind, PTY marker, and
read/cancel handles; `bash_tasks` also exposes the background task list as
`terminal_tasks`. Tool execution summaries now copy compact terminal task
status metadata into machine-readable `tool_summary` fields, so traces can
inspect shell task state without parsing provider text. `/status` now also
shows a read-only terminal task count by running/completed/failed/cancelled/
timed-out state. Obvious interactive commands such as bare `python3`,
`node -i`, bare `ssh` sessions, and `npm init` are now classified as requiring
PTY support; non-PTY bash returns a structured `mode=pty` recovery diagnostic
instead of starting a command it cannot control. `bash mode=pty` now runs
foreground commands through a `portable-pty` backend and records
`terminal_requirement.pty_used=true` in the tool result. PTY execution now uses
the same non-login `bash -c` command shape as foreground bash, avoiding hangs
from user login-shell startup files during short PTY commands.

Current file-quality slice: `file_read` and `grep` now preserve a clearer
raw/display boundary in structured tool data. File reads record path, resolved
path, displayed line range, total/displayed line counts, truncation state,
content hashes, and whether visible content contains line-number display
prefixes. Grep records search kind, display format, raw match lines, line
ranges, byte offsets, and line hashes. EvidenceLedger keeps those file-fact
metadata fields so closeout and later repair logic can use structured facts
instead of relying only on rendered text.
Read state now distinguishes full-file reads from targeted line-range reads.
With `PRIORITY_AGENT_SMART_EDIT=1`, exact/insert edits require a full read,
while line-range edits are allowed when the requested range is covered by a
previous targeted read. File state is now owned by `FileStateTracker`, and
file read/edit/write metadata exposes lexical, resolved, canonical, display,
and state-key path identity so relative, absolute, and canonicalized paths
share the same stale-read boundary. File reads now expose `text_format`
metadata for encoding, BOM, and line ending; edits and writes preserve UTF-8
BOM, UTF-16LE BOM, and LF/CRLF style instead of normalizing files by accident.
File mutations now share a per-canonical-path async lock, and text writes use
a temporary sibling file plus rename so same-file edits are serialized and
write failures do not leave partial file contents. `file_patch` records the
actual encoded bytes written for each patched file, so UTF-16LE/BOM file
history matches disk bytes instead of normalized UTF-8 string length. Each
successful `file_patch` file now also enters EvidenceLedger as changed-file
evidence with patch kind, changed line range, diff truncation state, and compact
bytes-written metadata. `file_patch` partial write failures now return structured
rollback evidence with failed path, checkpoint metadata, written-before-failure
paths, rollback success, and restored/removed/failed rollback files. `file_edit`
success results now include additions,
deletions, changed line range, and a bounded unified diff preview so later
closeout and diagnostics paths can cite actual edit evidence. `file_edit` also
returns a non-blocking `diagnostics` summary with first-error, first-warning,
affected-line-range, and compact EvidenceLedger first-error metadata. It samples
cached LSP diagnostics only from already-initialized clients, avoids triggering
slow language-server startup on the edit path, and records compact LSP
status/counts in EvidenceLedger file facts. The LSP sync path now tracks
documents already sent through `textDocument/didOpen`; follow-up edits use
`didChange` plus `didSave` with monotonic document versions instead of repeating
`didOpen`.

Validated locally with:

```bash
cargo fmt --check
cargo test -q file_tool -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q lsp -- --test-threads=1
cargo test -q tool_result -- --test-threads=1
cargo test -q test_bash_tool_pty_mode_runs_with_tty_stdout -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo check -q
cargo test -q
cargo clippy --all-features -- -D warnings
bash scripts/workflow-production-gates.sh
```

Latest expanded live-eval checkpoint:

```text
batch6-harnesssplit-20260511-155208 resume-session-picker: ok
diff=yes agent_required_commands=2 harness_commands=1 required_command_status=ok
verification_passed=true stage_validation_passed=true
acceptance_accepted=true closeout_status=passed failure_owner=none
note=full-suite cargo test is now harness-only for this case, keeping agent validation focused while preserving release-level evidence
batch6-evidencefix2-20260511-173535 cli-scrollback-polish: ok
intent=audit_or_regression_check diff=no required_command_status=ok
agent_required_commands=2 harness_commands=1 tool_errors=0 closeout_status=passed failure_owner=none runtime_validation=passed:2/2
note=agent and harness validation environments now agree; audit/no-diff closeout remains valid while runtime validation no longer reports stale recovered failures
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
| Extract tool orchestrator helpers | this change |
| Surface companion helper context | `f33174a` |
| Give focused repair two targeted lookups | this change |
| Add next-stage productization plan | this change |
| Add reference audit and architecture map | this change |
| Normalize pure tool-call assistant messages for strict providers | this change |

The all-features clippy and experimental API checks are clean as of this
focused lookup-budget change.

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

Latest capability live-suite run:

```text
capability-evidence-20260509-173239: 6/6 passed, all real code-change passes
cases=code-change-verification-repair-loop, live-eval-dashboard-summary,
backend-todo-api-crud, frontend-book-notes-localstorage,
memory-save-quality-gate, skill-promotion-gate
memory_active_tasks=6 memory_changed_plan_tasks=5
skill_active_tasks=1 skill_promotion_evidence_tasks=1
note=live-eval-dashboard-summary recovered from invalid action checkpoint before passing
```

Latest dashboard model-led repair rerun:

```text
dashboard-patch-retry-20260509-200245 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true acceptance_accepted=true closeout_status=passed failure_owner=none
model_file_edit=true patch_synthesis_used=false first_write_tool_index=5
warnings=tool_errors_seen,earlier_verification_failed_before_repair,earlier_stage_validation_failed_before_repair
rerun_after=8d4658b targeted file-read cache fix, 4f4aa8f/ea337e6/cd31b56 checkpoint deferral and retry
note=model produced and repaired its own edits; deterministic patch synthesis did not take over
```

Latest dashboard focused-repair lookup-budget rerun:

```text
focused-lookup-budget-20260509-212938 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true acceptance_accepted=true closeout_status=passed failure_owner=none
model_file_edit=true patch_synthesis_used=false first_write_tool_index=6
tool_errors=0 tool_failures=3 changed_files=1
note=model used two targeted read/search rounds, consumed a line-range correction after a failed edit, then produced its own edit without deterministic patch synthesis
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
- Code-change turns now surface concise companion-context hints after targeted
  `file_read`/`grep` evidence when nearby helper/parser files strongly match
  the inspected file and task. This keeps helper discovery in runtime evidence
  instead of adding more always-on prompt rules.
- Bash command failures now add a concrete compatibility hint for macOS bash
  3.x associative-array errors (`declare -A`), steering repair toward portable
  shell, awk/temp-file, or existing Python helper paths.

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
   conversation-loop extraction are now landed. Batch 5 has started with
   explicit coding agent modes, mode-visible status, and stronger `/doctor`
   tool-exposure diagnostics. Batch 6 report-layer memory/skill evidence is
   now landed in live summary and aggregate reporting, and has a first
   six-case live baseline. Behavior-level memory/skill assertion metadata is
   now part of the live-eval report layer, and the affected recommended
   memory/skill cases have been rerun. The current rerun shows the audit/no-diff
   memory cases passing, and the three previously failing seeded memory/skill
   code-change cases now have a targeted `3/3` rerun with required commands,
   behavior assertions, and closeout passing. A full six-case rerun can refresh
   the combined recommended baseline, but the previous blockers are no longer
   active.
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

- `core-quality-real-rerun-20260517-091952` refreshed the real
  `core-coding-quality` agent-run baseline on 2026-05-17: `8/8` passed,
  required commands were `ok` for every case, failure owner was `none` for every
  case, `3` seeded code-change tasks produced real diffs, and `5` audit/no-diff
  tasks passed as expected.
- `product-maturity-memory-skill-20260517-102935` refreshed the affected
  recommended memory/skill behavior-assertion baseline on 2026-05-17: `5/6`
  passed, `5/6` behavior assertion tasks passed, and
  `persistent-memory-planning-context` failed with `failure_owner=agent_flow`.
  The stale fixture path was updated from the old main loop to
  `TurnRetrievalContextController`; the remaining failure is a real runtime
  flow issue where reflection permission stopped the seeded code-change task
  before tools, leaving the required memory prefetch, merge, and trace
  assertions unrestored.
- `persistent-reflection-fix-20260517-113556` fixes that pre-tool reflection
  stop path: entry-gate task context now treats extracted required validation
  commands as acceptance checks before the reflection gate runs. The targeted
  rerun passed with `required_command_status=ok`,
  `behavior_assertion_status=passed`, `failure_owner=none`, one real code diff,
  and isolated full-suite evidence of `1438 passed; 0 failed`. The later
  recommended memory/skill rerun is the current broader baseline and shows that
  the targeted fix did not fully generalize.
- `product-maturity-memory-skill-rerun-20260517-124722` reran the six affected
  recommended memory/skill cases after the file-patch write guard and
  live-eval scoring fixes: `3/6` passed, all three audit/no-diff memory cases
  passed, and all three seeded code-change cases failed. The failures are
  `memory-save-quality-gate` (`failure_owner=agent_flow`,
  `closeout_not_successful`), `skill-promotion-gate`
  (`failure_owner=agent_flow`, required commands and behavior assertions
  failing), and `persistent-memory-planning-context`
  (`failure_owner=llm_reasoning`, required commands, behavior assertions,
  stage validation, verification, acceptance, and closeout failing).
- `product-maturity-seeded-fixes-20260517-143047` reran the three previously
  failing seeded memory/skill code-change cases after the closeout and
  deterministic patch-rule priority fixes: `3/3` passed, all three produced
  real diffs, all required commands were `ok`, all behavior assertions passed,
  closeout passed, and `failure_owner=none` for every case.
- Patch synthesis now gives high-confidence deterministic repair rules first
  refusal when they match current evidence. This prevents a valid-but-wrong
  model JSON patch from overriding known regression repairs such as
  `memory-save-quality-gate` and `skill-promotion-gate`.
- Required-validation acceptance can now produce a deterministic accepted
  review from task-bundle acceptance checks when all required validation
  commands pass but workflow judgment was skipped after a non-JSON model
  response. This keeps closeout from leaving required-command checks pending
  after successful validation.
- Live-eval summaries now carry explicit `behavior_assertions` and
  `behavior_assertion_status` fields. The first product-maturity slice tags the
  memory and skill recommended tasks so summary and aggregate reports can show
  memory/skill behavior coverage separately from memory/skill activity signals.
- `file_patch` write-mode operations now honor the same existing-file
  read-before-edit and stale-read checks as targeted patch operations, and the
  live-eval report parser now marks unverified collect-only reports as
  `skipped` instead of counting them as passed.
- `cargo test -q` is clean as of 2026-05-17 with `1444 passed; 0 failed`.
- Provider API calls now use a bounded reconnect policy for transient transport
  failures. `PRIORITY_AGENT_PROVIDER_RECONNECT_ATTEMPTS` defaults to `5`
  reconnect opportunities, with exponential backoff, and does not retry
  auth/schema/400-class request-contract failures.
- Provider health preflight is now protocol-focused: plain chat, tool-call, and
  tool-result continuation must work, but the continuation probe no longer
  requires the model to repeat an exact Closeout phrase before live evals can
  start.
- Latest validation for the companion-context slice: `cargo fmt --check`,
  `cargo test -q companion_context`, `cargo test -q shell_compatibility_hint`,
  `cargo test -q agent_mode`, `cargo check -q`, `cargo test -q`,
  `cargo clippy --all-features -- -D warnings`,
  `cargo check --features experimental-api-server -q`, and `git diff --check`.
- Latest validation for the focused lookup-budget slice:
  `cargo fmt --check`, `cargo test -q focused_repair_prompt`,
  `cargo test -q file_edit_failure_correction`, `cargo check -q`,
  `cargo test -q`, `cargo clippy --all-features -- -D warnings`,
  `cargo check --features experimental-api-server -q`,
  `bash -n scripts/run_live_eval.sh`,
  `bash -n scripts/live-eval-aggregate-summary.sh`,
  `bash scripts/live-eval-summary-smoke.sh`, `git diff --check`,
  and `scripts/run_live_eval.sh --case live-eval-dashboard-summary --mode agent-run --run-tests --label focused-lookup-budget --overlay-working-tree`.
- Batch 5 product mode work has an explicit runtime `AgentMode`
  (`auto/build/plan/explore/review`) that flows from TUI `/mode` into
  streaming and `ConversationLoop` route/tool exposure. `/status`, `/quick`,
  status bar, and `/doctor` now show the current mode; `/doctor` also reports
  how the current mode affects bash and write-tool visibility.
- Validation for the Batch 5 mode slice: `cargo fmt --check`,
  `cargo test -q agent_mode`, `cargo test -q mode_`,
  `cargo test -q doctor_route_summary_applies_agent_mode_before_exposure_checks`,
  `cargo test -q status`, `cargo test -q quick`, `cargo test -q tool_view`,
  `cargo check -q`, and `cargo test -q`.
- Batch 6 reporting now surfaces memory/skill evidence in
  `scripts/run_live_eval.sh --mode summary` and
  `scripts/live-eval-aggregate-summary.sh`: memory active tasks, recalled
  items, conflict counts, changed-plan signals, skill active tasks, usage
  events, and promotion-evidence tasks. Validation: `python3 -m py_compile
  scripts/live_eval_report_parser.py`, `bash -n scripts/run_live_eval.sh`,
  `bash -n scripts/live-eval-aggregate-summary.sh`,
  `bash scripts/live-eval-summary-smoke.sh`, `cargo test -q memory`,
  `cargo test -q retrieval_context`, `cargo test -q skills`,
  `bash scripts/coding-workflow-gates.sh standard`, and `cargo check -q`.
- The latest six-case live capability suite is
  `docs/benchmarks/live-capability-evidence-20260509-173239/summary.md` with
  `6/6` passed real code-change tasks. During this pass, stale live-eval
  fixtures were refreshed for the extracted repair controller and learning
  slash-handler modules, and skill-promotion evidence detection was widened so
  `skill-promotion-gate` is counted as a skill-specific task.
- `conversation_loop/mod.rs` is down to 3159 lines after moving turn setup,
  entry gates, loop bootstrap, iteration sequencing, tool-round sequencing,
  post-change closeout, retrieval helpers, workflow-runtime helpers, and
  tool-context helpers into dedicated conversation-loop modules, then moving
  route-scoped tool exposure tests out of the main module.
- `cargo clippy --all-features -- -D warnings` was last recorded clean on
  2026-05-16 after the tool-context helper split.
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
- Focused repair prompts now consistently allow up to two targeted
  `file_read`/`grep` lookups before patching instead of contradicting that with
  a blanket read/search ban. Action-checkpoint unexposed-tool errors now list
  the currently exposed tools and the expected repair path.
- Action checkpoint now enforces that targeted lookup budget in the exposed
  tool set: after the budget is used, the next focused repair request hides
  read/search tools and forces patch tools only.
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
- Action-checkpoint patch repair now records patch synthesis `source` plus
  owner/reason in traces. When a high-confidence deterministic repair rule
  matches current evidence, deterministic patch synthesis runs before model
  JSON/tool-call synthesis. Model synthesis remains the fallback for evidence
  without a matching deterministic rule.
- `/status` and `/doctor` terminal diagnostics now include bash provider-schema
  compatibility (`schema=ok` or a concrete schema failure reason) alongside
  registry, availability, permission, and route exposure checks.
- Bash command classification now has a finer `ShellCommandCategory` shared by
  bash result metadata, evidence, tool summaries, and progress labels. Ordinary
  `rg ...` commands are search operations; only explicit `! rg ...` assertions
  count as required validation.
- Bash permission risk now uses that shared category: read/list/search and
  validation commands are low risk, while package install, dev server, file
  mutation, git mutation, network, outside-workspace, and destructive commands
  keep stronger confirmation behavior.
- TUI bash summaries now reuse the shared classifier, so terminal UI labels for
  listing, search, validation, package install, dev server, git mutation, and
  shell mutation match runtime evidence and permission semantics.
- Bash tool results now include structured `shell_result` metadata and store
  long combined output under `.priority-agent/tool-results/<session>/...` while
  keeping provider-facing output bounded to a preview. Tool execution metadata
  fills `shell_result.duration_ms` after the controller measures elapsed time.
- Code-change workflow strictness is now adaptive instead of medium-risk by
  default: required validation, first code change, failed verification,
  acceptance rejection, and repeated no-edit progress activate the heavier
  judgment/validation/repair path automatically. Closeout evidence records the
  trigger labels so benchmark reports can explain why strict mode engaged.
- Adaptive workflow triggers are first-class trace events and live-eval
  summaries now expose a `triggers` column plus aggregate trigger distribution,
  so strict-mode activation can be measured without parsing fallback prose.
- Audit/regression live evals now route through the code workflow without
  requiring arbitrary diffs, bash child processes strip agent runtime env vars
  before running validation commands, and workflow judgment factor parsing
  tolerates missing optional fields. After the reconnect policy and
  protocol-only provider health update, `batch6-reconnect-20260511-132912`,
  `batch6-reconnect-20260511-133851`, and
  `batch6-reconnect-20260511-135823` passed as audit/no-diff checks with
  required commands ok, full `1195 passed; 0 failed`,
  `closeout_status=passed`, and `failure_owner=none`.
- The expanded 12-case recommended suite now has current passing evidence.
  Cases 8-10 passed as audit/no-diff checks in the reconnect batch.
  `resume-session-picker` passed in `batch6-harnesssplit-20260511-155208`
  after focused agent-visible validation was split from harness-only full-suite
  validation. `cli-scrollback-polish` passed in
  `batch6-evidencefix2-20260511-173535` after runtime validation labels and
  live-eval provider environments were aligned.
- Provider health preflight is now available as
  `priority-agent --provider-health` and is enabled by default for
  `scripts/run_live_eval.sh --mode agent-run`. It probes plain chat, tool-call,
  and tool-result continuation before spending a live-eval run; failures are
  written as provider-health artifacts and task reports classify the stop as an
  environment/provider issue. Use `--skip-provider-health` or
  `PRIORITY_AGENT_LIVE_EVAL_PROVIDER_HEALTH=0` only for debugging the gate
  itself.

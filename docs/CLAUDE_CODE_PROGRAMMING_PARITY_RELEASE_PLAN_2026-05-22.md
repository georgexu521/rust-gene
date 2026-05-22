# Claude Code Programming Parity Release Plan

Date: 2026-05-22

Status: follow-up execution plan. This document starts after
`docs/CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md` reached local
deterministic replay readiness and external-baseline ingestion surfaces.

Goal: move Priority Agent from "Claude Code-inspired and usable for controlled
coding tasks" to "stable enough to ship as a daily coding-agent product with
Claude Code-like programming ability."

This plan focuses on programming ability: tool calling, file editing, bash and
terminal tasks, permissions, subagents, context handling, TUI feedback, and
release evidence. It is not a generic feature-count race.

## Product Target

The target experience is:

1. A user can open the CLI in a real repo and ask for a code change.
2. The agent reads the right files, edits safely, runs relevant validation, and
   reports evidence without repeated steering.
3. Tool execution order, permission review, terminal tasks, file history,
   diffs, context pressure, and subagent work are visible as product state.
4. Failures are recoverable: stale reads, denied permissions, long-running
   commands, provider protocol issues, context-too-long errors, MCP auth
   failures, and bad edits produce specific recovery paths.
5. The product can be installed, updated, diagnosed, and regression-tested
   without depending on ad hoc local knowledge.

Approximate current position from the 2026-05-22 source comparison:

| Area | Current maturity | Release target |
|------|------------------|----------------|
| Local coding loop | 70% | 90% |
| File edit/history/rewind | 70-75% | 90% |
| Bash/terminal task behavior | 60-65% | 90% |
| Tool-call contract and orchestration | 55-60% | 90% |
| Permissions/hooks/human review | 65-70% | 85% |
| Subagents/worktrees | 55-60% | 80-85% |
| Context/memory/skills | 55-60% | 80-85% |
| TUI/product UX | 35-45% | 85% |
| MCP/plugins/bridge/provider polish | 35-45% | 75-80% |
| Release/install/diagnostics | 40-50% | 85% |

The important point: the next gap is not missing nouns. It is runtime semantics,
durable state, UI feedback, and empirical proof.

## Reference Rule

Before implementing a major subsystem, re-read the current local Claude source
for that subsystem:

- Tool contract: `/Users/georgexu/Desktop/claude/src/Tool.ts`
- Tool execution: `/Users/georgexu/Desktop/claude/src/services/tools/`
- Bash: `/Users/georgexu/Desktop/claude/src/tools/BashTool/`
- File tools: `/Users/georgexu/Desktop/claude/src/tools/File*Tool/`
- File history: `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts`
- Agent tool: `/Users/georgexu/Desktop/claude/src/tools/AgentTool/`
- Tasks: `/Users/georgexu/Desktop/claude/src/tasks/`
- App state: `/Users/georgexu/Desktop/claude/src/state/`
- UI: `/Users/georgexu/Desktop/claude/src/components/`
- Context/compact/memory/skills: `/Users/georgexu/Desktop/claude/src/services/`
  and `/Users/georgexu/Desktop/claude/src/skills/`
- MCP/plugins/bridge/remote:
  `/Users/georgexu/Desktop/claude/src/services/mcp/`,
  `/Users/georgexu/Desktop/claude/src/plugins/`,
  `/Users/georgexu/Desktop/claude/src/bridge/`,
  `/Users/georgexu/Desktop/claude/src/remote/`

Translate product semantics into original Rust code. Do not copy Claude source
or long prompt/UI strings verbatim.

## Semantic Replication Protocol

The goal is not clean-room invention when Claude Code already contains a better
programming-agent pattern. The goal is also not source-code copying. The working
method for this plan is semantic replication:

1. Read the relevant Claude module immediately before implementing a Priority
   Agent subsystem.
2. Extract the behavior as a small design note:
   - inputs and outputs;
   - state carried between calls;
   - ordering guarantees;
   - concurrency boundaries;
   - permission and hook checkpoints;
   - error branches and fallback behavior;
   - result normalization and UI facts;
   - tests or examples needed to prove parity.
3. Implement the same semantics in Rust using Priority Agent's own types,
   module boundaries, error handling, and storage model.
4. Add tests that assert behavior, not textual similarity.
5. Record any deliberate divergence in the plan or status doc.

### What Can Be Replicated

These are legitimate parity targets and should be copied at the semantic level:

- Function responsibility boundaries.
- State-machine shape.
- Tool-call ordering algorithms.
- Validation and permission checkpoint order.
- Error classification and recovery decision trees.
- File edit safety sequence: normalize path, validate input, verify read state,
  detect stale content, prepare checkpoint, mutate, record diff, update state.
- Bash safety sequence: parse command, classify subcommands, cap expensive
  analysis, build permission facts, choose foreground/background/PTY behavior,
  record task output.
- Subagent fork semantics: preserve parent context, insert deterministic
  placeholder tool results, add child directive, prevent uncontrolled recursive
  forking, isolate mutating work in a worktree.
- Context compaction semantics: record boundary, preserve active task facts,
  retain critical tool/file/validation evidence, emit traceable provenance.
- UI product structure: status, diff, approval, task, agent, context, MCP, and
  trace panels backed by runtime state.
- Test scenarios and acceptance criteria.

### What Must Not Be Copied

Do not copy:

- Source code bodies.
- Long prompt strings or UI copy.
- Private identifiers that do not make sense in Priority Agent.
- Analytics names, internal experiment names, or vendor-specific feature flags.
- Exact file organization when the Rust codebase already has a better local
  ownership boundary.
- Dead code, compatibility branches, or product experiments that are not needed
  for Priority Agent's release target.

### Claude-First Implementation Checklist

For each parity batch, create or keep a short local note in the PR/commit
description or plan progress section:

```text
Claude reference:
- files/functions inspected:
- behavior extracted:
- Priority Agent target files:
- intentional divergences:
- parity tests added:
- validation commands:
```

This makes Claude Code a practical implementation reference while keeping the
Priority Agent implementation original and maintainable.

### Examples From The Current Gap

Tool orchestration should follow Claude's algorithmic shape: group consecutive
concurrency-safe tool calls, run each read-only group concurrently, run
mutating or unsafe calls serially, and apply state/context changes in original
tool-call order. Priority Agent should not merely "support parallel tools"; it
should match this ordering guarantee.

File edit should follow Claude's safety sequence: normalize and canonicalize the
path, reject impossible edits before permission, check permission rules, verify
the file has been read, reject stale content unless content is unchanged,
handle exact-match ambiguity, prepare a checkpoint/history record, write while
preserving text format, then update read/file state and emit diff evidence.

Bash permission should follow Claude's risk-shaping approach: parse before
trusting the command, cap expensive compound-command analysis, generate stable
rule suggestions, avoid broad shell-wrapper approvals, and fall back to asking
when safety cannot be proven.

Subagent fork should follow Claude's context strategy: fork with deterministic
placeholder results for parent tool calls, keep child output constrained,
prevent recursive uncontrolled delegation, and use isolated worktrees for
mutating parallel work.

## Release Gates

### Internal Alpha

The product is ready for daily dogfooding by gex when:

- Ordered tool execution matches Claude-like read/write semantics.
- File edits create durable checkpoint-backed evidence.
- Bash foreground/background/PTY tasks are visible and recoverable.
- `/status`, `/tool-output`, `/diff`, `/rewind`, `/permissions`, `/tasks`,
  `/agents`, `/context`, and `/trace` are usable without reading raw logs.
- Real Claude Code and Codex artifacts have been imported for the parity matrix.
- Full local tests and clippy are clean.

### Beta

The product is ready for a small external tester group when:

- The real-project coding gauntlet is stable across repeated runs.
- Install/update/doctor flows are reliable on a fresh machine.
- Provider protocol failures are typed and recoverable.
- TUI panels do not expose placeholder flows as production features.
- Permission and destructive action review are understandable without source
  knowledge.
- Crash/failure reports can be exported with enough evidence to debug.

### Release Candidate

The product is ready for release candidate status when:

- A pinned parity suite shows no P0 gaps against Claude Code for local repo
  coding tasks.
- The product can survive long sessions with compaction, background tasks, and
  subagents without losing state.
- Release packaging, migration, config, and rollback paths are documented and
  tested.
- Known non-parity decisions are documented as deliberate product choices, not
  accidental gaps.

## Phase 0: Evidence Lock And External Baseline

Purpose: stop planning from drifting and get real comparison data early.

Tasks:

1. Generate real Claude Code baseline artifacts for the six Phase 12 scenarios.
2. Generate real Codex baseline artifacts for the same scenarios.
3. Import them with `/eval baseline-import`.
4. Validate with `/eval baseline-validate`.
5. Record reports with `/eval parity-record`.
6. Add a second wider set of 10-15 real coding tasks:
   - small bug fix
   - cross-file refactor
   - test failure repair
   - frontend UI change
   - CLI behavior change
   - permission-denied recovery
   - long-running dev server or watcher
   - package install refusal/approval path
   - stale-read edit conflict
   - subagent worker review/merge
   - context compaction during a long turn
   - MCP auth/resource retry

Deliverables:

- Imported baseline files under `evalsets/external_baselines/`.
- Recorded parity reports under `target/eval-reports/`.
- A short current-gap summary appended to the parity plan or status doc.

Acceptance:

- No parity claim relies only on deterministic local replay.
- Each passing external scenario has transcript/evidence notes, not just a
  hand-written pass label.

## Phase 1: Tool-Call Orchestration Semantics

Purpose: match Claude-like tool execution ordering before deepening more tools.

Current gap:

- Claude partitions tool calls into consecutive concurrency-safe read batches
  and serial mutating calls, preserving model order.
- Priority Agent currently collects safe read-only calls and then executes
  mutating calls. This can change the meaning of `read -> edit -> read`.

Tasks:

1. Replace global read-first scheduling with ordered partitioning:
   - consecutive concurrency-safe calls form one concurrent batch;
   - each mutating or non-safe call forms a serial batch;
   - batch order follows the assistant message order.
2. Treat Claude's partitioning flow as the behavioral reference, but implement
   it as original Rust batch types owned by `ToolExecutionController`.
3. Preserve pre-executed provider read-only results without changing relative
   order.
4. Return tool results in the model-visible order required by provider
   protocols.
5. Keep lifecycle records, trace events, permission events, and runtime tool
   status aligned with the ordered batches.
6. Add regression tests for:
   - `read, write, read`
   - `read, read, write, read, read`
   - denied tool between read batches
   - pre-executed read-only before and after a write
   - max-tool-call policy with ordered batches

Primary files:

- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_execution.rs`
- `src/engine/conversation_loop/tool_call_lifecycle.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`

Validation:

- `cargo test -q tool_execution`
- `cargo test -q tool_call_lifecycle`
- `cargo test -q provider_protocol`
- `cargo check -q`

Acceptance:

- Tool call ordering is semantically equivalent to Claude Code for mixed
  read/write rounds.
- Parallelism is retained only inside safe consecutive read batches.

Progress, 2026-05-22:

- Implemented ordered tool execution in `ToolExecutionController`: consecutive
  concurrency-safe calls run as bounded concurrent read batches, while mutating
  or unsafe calls execute serially at their original position.
- Tool results are returned in assistant tool-call order, including read-only
  batches that complete out of order internally.
- Pre-executed read-only results now keep their original position when they are
  still before a serial boundary; pre-executed reads after a write/serial
  boundary are rerun in order so they cannot observe stale pre-write state.
- Tool lifecycle completion preserves the earlier parallel/pre-executed flags
  instead of resetting them on completion.
- Added regression coverage for:
  - `read -> write -> read`
  - `read -> read -> write -> read -> read`
  - denied tool between read batches
  - pre-executed read-only result before a write
  - pre-executed read-only result after a write reruns in order
- Validation:
  - `cargo test -q tool_execution` - passed, 24 tests.
  - `cargo test -q tool_call_lifecycle` - passed, 2 tests.
  - `cargo test -q provider_protocol` - passed, 12 tests.
  - `cargo test -q closeout` - passed, 23 tests.
  - `cargo fmt --check` - passed.
  - `cargo check -q` - passed.
  - `cargo test -q` - passed, 1577 tests.
  - `cargo clippy -q -- -D warnings` - passed.
  - `cargo check --features experimental-api-server -q` - passed.
  - `cargo clippy --all-features -q -- -D warnings` - passed.
  - `git diff --check` - passed.

## Phase 2: Tool Contract Completion

Purpose: make tools product objects, not just JSON executors.

Already present:

- Basic schema, execution, output schema hook, operation kind, read-only,
  concurrency-safe, destructive, max result size, user-facing name, summary,
  activity label, provider payload, classifier input.

Remaining parity work:

1. Add or finish contract concepts:
   - aliases
   - search hints
   - deferred loading / tool search visibility
   - strict-schema capability
   - `interrupt_behavior`
   - `requires_user_interaction`
   - `is_open_world`
   - `is_search_or_read_command`
   - path extraction
   - permission matcher preparation
   - observable input backfill for hooks/transcript
   - transcript/search summary extraction
   - UI render metadata hooks
2. Deepen the core tools first:
   - `file_read`
   - `file_edit`
   - `file_write`
   - `file_patch`
   - `bash`
   - `bash_output`
   - `bash_cancel`
   - `grep`
   - `glob`
   - `todo_write`
   - `agent`
   - `task_*`
3. Store all tool execution facts in one durable record path.
4. Use records for provider payload, TUI rows, closeout evidence, trace, and
   replay assertions.

Primary files:

- `src/tools/mod.rs`
- `src/engine/evidence_ledger.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/tui/tool_view.rs`

Validation:

- `cargo test -q tool_contract`
- `cargo test -q evidence_ledger`
- `cargo test -q tool_result`
- `cargo test -q closeout`
- `cargo check -q`

Acceptance:

- A tool's runtime behavior, permission review, UI summary, and final evidence
  come from structured tool facts, not string parsing.

Progress, 2026-05-22:

Claude reference:

- Inspected `/Users/georgexu/Desktop/claude/src/Tool.ts`,
  `/Users/georgexu/Desktop/claude/src/tools/ToolSearchTool/prompt.ts`,
  `/Users/georgexu/Desktop/claude/src/services/api/claude.ts`,
  `/Users/georgexu/Desktop/claude/src/utils/api.ts`,
  `/Users/georgexu/Desktop/claude/src/services/tools/StreamingToolExecutor.ts`,
  and `/Users/georgexu/Desktop/claude/src/services/tools/toolHooks.ts`.
- Extracted behavior: tools carry aliases, search hints, defer/always-load
  visibility, strict-schema capability, interrupt behavior,
  user-interaction gating, search/read/list semantics, open-world hints,
  observer-only input backfill, and provider/UI metadata separate from the
  raw executor.

Priority Agent implementation:

- Added Tool Contract V2 completion fields to `Tool`/`ToolSchema`: aliases,
  search hints, `should_defer`, `always_load`, `strict_schema`,
  `interrupt_behavior`, `requires_user_interaction`, `is_open_world`,
  `is_search_or_read_command`, path extraction, permission matcher input,
  observable input backfill, transcript summary, and UI render kind.
- `ToolRegistry` now resolves aliases, and `tool_search` ranks aliases plus
  search hints and supports `select:a,b` canonical lookup.
- Core tools deepened in this batch:
  - `file_read`, `file_write`, `file_edit`, `file_patch`
  - `bash`, `bash_output`, `bash_cancel`, `bash_tasks`
  - `grep`, `glob`
  - `todo_write`
  - `agent`
  - `task_create`, `task_get`, `task_list`, `task_update`, `task_stop`,
    `task_output`
  - `ask_user`, `exit_plan_mode`, `tool_search`
- Tool contract metadata is now attached to both `tool_contract` and compact
  `tool_summary` records; `EvidenceLedger::ToolExecutionRecord` preserves the
  same facts for closeout/repair/replay consumers.
- Permission request metadata now records the structured matcher input, input
  paths, open-world flag, search/read/list classification, and UI render kind.
- Provider API tool definitions now preserve strict-schema intent; actual
  OpenAI-compatible/Kimi strict emission is gated behind
  `PRIORITY_AGENT_ENABLE_STRICT_TOOL_SCHEMA` until provider/schema support is
  proven broadly enough for a safe default.
- Runtime/TUI state now carries operation kind, UI render kind, read-only,
  concurrency-safe, destructive, input paths, and transcript summary from
  tool metadata.

Intentional divergences:

- Priority Agent keeps provider-specific `defer_loading` as internal
  visibility metadata for now. The project does not yet send Anthropic beta
  `defer_loading` tool schemas because the active provider abstraction is
  OpenAI-compatible/Kimi-oriented.
- Strict tool schema emission is also opt-in for now; Claude gates this by
  provider/model support and feature flags, while Priority Agent still needs a
  schema-normalization pass before enabling it by default.
- Existing polished TUI summaries remain in place for core tools; structured
  metadata is now available in runtime state and records without replacing all
  hand-tuned text in one batch.

Validation:

- `cargo test -q tool_contract` - passed, 6 tests.
- `cargo test -q tool_metadata` - passed, 8 tests.
- `cargo test -q tool_search` - passed, 3 tests.
- `cargo test -q evidence_ledger` - passed, 20 tests.
- `cargo test -q tool_result` - passed, 33 tests.
- `cargo test -q closeout` - passed, 23 tests.
- `cargo test -q command_classifier` - passed, 7 tests.
- `cargo test -q bash_tool` - passed, 30 tests.
- `cargo test -q task_tool` - passed, 2 tests.
- `cargo test -q agent_tool` - passed, 11 tests.
- `cargo test -q permission_controller` - passed, 8 tests.
- `cargo test -q openai_compat` - passed, 7 tests.
- `cargo test -q kimi` - passed, 4 tests.
- `cargo test -q runtime_state` - passed, 4 tests.
- `cargo test -q runtime_panels` - passed, 5 tests.
- `cargo test -q test_runtime_snapshot_keeps_terminal_task_metadata` - passed.
- `cargo test -q tool_execution` - passed, 24 tests.
- `cargo check -q` - passed.
- `cargo clippy -q -- -D warnings` - passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Phase 3: Bash And Terminal Productization

Purpose: make shell execution safe, visible, and reliable enough for daily
coding work.

Tasks:

1. Replace broad string matching with a safer parser/classifier path for shell
   commands where possible.
2. Mirror Claude's command-safety decision tree at the behavior level:
   - cheap normalization first;
   - bounded parsing for compound commands;
   - conservative fallback to ask when safety cannot be proven;
   - stable rule suggestion only when the prefix is specific enough.
3. Expand command facts:
   - search/read/list
   - validation/test
   - dev server
   - package install
   - git mutation
   - file mutation
   - destructive
   - network access
   - external path access
   - PTY required
   - expected silent output
4. Add command-specific permission review:
   - stable rule pattern suggestions;
   - exact-command and prefix-command options;
   - deny dangerous shell wrappers;
   - explain shell risk in the approval panel.
5. Implement Claude-like task output behavior:
   - durable output file for large/long-running shell output;
   - preview plus path;
   - `bash_output` reads by task id;
   - `bash_cancel` cancels by task id;
   - `/tasks` and `/status` show task state.
6. Add auto-background policy for long blocking commands with a configurable
   threshold.
7. Improve PTY recovery:
   - detect interactive commands;
   - tell the model to retry with PTY or foreground when needed;
   - avoid hanging non-interactive sessions.
8. Track git/file mutations from shell commands where feasible.

Primary files:

- `src/tools/bash_tool/`
- `src/task_manager/`
- `src/state/runtime_state.rs`
- `src/tui/runtime_panels.rs`
- `src/tui/tool_view.rs`
- `src/permissions/`

Validation:

- `cargo test -q command_classifier`
- `cargo test -q bash_tool`
- `cargo test -q shell_lifecycle`
- `cargo test -q runtime_state`
- `cargo test -q permissions`
- `cargo check -q`

Acceptance:

- Long shell commands do not disappear.
- Validation commands become machine-readable evidence.
- Risky shell commands produce understandable, command-specific review.

Progress on 2026-05-22:

- Started Phase 3 with the shell-command classifier and metadata path.
- Added machine-readable command facts for network access, external/absolute
  path access, compound shell operators, risky shell wrappers, PTY requirement,
  expected silent output, and exact/prefix permission rule suggestions.
- Added `cargo fmt --check` as a validation family and classified `cargo fmt`
  without `--check` as a file mutation.
- Expanded package/network detection for `npm ci`, Go install/get, git
  clone/fetch/submodule, and common remote/network tools.
- Propagated the new command facts into bash result metadata, provider tool
  summaries, permission review records, evidence ledger command facts, and tool
  execution records.
- Permission explanations now warn on shell network access, risky shell
  wrappers, and commands whose successful execution may be silent.
- Added a conservative auto-background policy for foreground dev-server/watch
  commands when the requested timeout meets
  `PRIORITY_AGENT_BASH_AUTO_BACKGROUND_SECS` (default 30s). It can be disabled
  with `PRIORITY_AGENT_BASH_AUTO_BACKGROUND=0` and records the auto-background
  reason in shell result, background task, and terminal-task metadata.
- Bash file/git mutation commands now contribute classifier path patterns to
  changed-path evidence when the shell command succeeds.
- Bash permission persistence now uses command-scoped rule keys instead of the
  broad `bash` tool name. Stable validation commands save as prefix rules such
  as `bash:cargo test*`; other shell approvals save exact command rules such as
  `bash:npm run dev`. Runtime permission matching keeps compatibility with old
  broad `bash` rules.
- The TUI approval preview for bash now shows command category/kind, validation
  family, risk flags such as network or PTY-required, visible path patterns, and
  the exact command-scoped rule that will be saved.
- `bash_output` and `bash_cancel` now accept `task_id` as an alias for
  `handle`, matching the terminal-task metadata returned by background bash.

Validation so far:

- `cargo test -q command_classifier` - passed, 8 tests.
- `cargo test -q tool_metadata` - passed, 8 tests.
- `cargo test -q permission_controller` - passed, 8 tests.
- `cargo test -q evidence_ledger` - passed, 22 tests.
- `cargo test -q bash_tool` - passed, 34 tests.
- `cargo test -q permissions` - passed, 51 tests.
- `cargo test -q human_review` - passed, 7 tests.
- `cargo test -q test_bash_session_permission_rule_uses_command_scope` -
  passed.
- `cargo test -q render_permission_approval_shows_bash_risk_and_decisions` -
  passed.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.
- `cargo clippy -q -- -D warnings` - passed.
- `git diff --check` - passed.

## Phase 4: File Tools, History, Diff, And Rewind

Purpose: make code mutation recoverable and inspectable.

Tasks:

1. Move file read state fully into `ToolUseContext`/runtime state:
   - content hash
   - mtime
   - canonical path
   - lexical path
   - line range
   - encoding
   - line endings
2. Mirror Claude's file-edit safety sequence as behavior:
   - expand and normalize the path;
   - reject invalid edits before writing;
   - apply permission checks before mutation;
   - require full or relevant read coverage;
   - compare stale state by mtime and content hash;
   - handle exact-match ambiguity and replace-all behavior;
   - preserve line endings and encoding;
   - update file state after the write.
3. Ensure every mutation creates checkpoint-backed evidence before writing.
4. Add turn-level file history snapshots, not only per-tool change records.
5. Make `/diff` and `/rewind` read from the same source of truth as file tools.
6. Add restore coverage for:
   - last file change
   - explicit file change id
   - checkpoint id
   - whole-turn rollback
7. Add optional editor/LSP/IDE notification hooks where available.
8. Strengthen edge cases:
   - empty file creation
   - multiple matches
   - partial reads
   - file changed by formatter after read
   - non-UTF8 or BOM files
   - very large files

Primary files:

- `src/tools/file_tool/`
- `src/engine/checkpoint.rs`
- `src/tools/diff_tool/`
- `src/tools/rewind_tool/`
- `src/session_store/`
- `src/state/runtime_state.rs`

Validation:

- `cargo test -q file_tool`
- `cargo test -q file_state`
- `cargo test -q checkpoint`
- `cargo test -q rewind`
- `cargo test -q diff`
- `cargo check -q`

Acceptance:

- Every meaningful file mutation has before/after evidence and a usable restore
  path.
- Final closeout can cite exact changed files and validation evidence from
  records.

Progress on 2026-05-22:

- The model-facing `rewind` tool now performs real checkpoint-backed restore
  operations instead of returning a placeholder success message. It can restore
  the latest file change, the Nth most recent file change through the legacy
  `steps` parameter, an explicit `file_change_id`, an explicit `checkpoint_id`,
  or the latest tracked change for a path.
- `rewind` uses the same `CheckpointManager` / `file_history.json` source as
  file tools and slash-command rewind, and returns structured restore metadata
  including restored, removed, and failed files.
- The model-facing `diff` tool now reads checkpoint-backed file history through
  `history` and `file_change` actions, so slash `/diff`, file tools, and tool
  calls share the same durable file-change source for recent mutation diffs.
- TUI slash paths for `/diff`, `/rewind`, `/checkpoints`, `/restore`, and
  `/rollback last-file` now use the actual active TUI session ID when reading
  checkpoints instead of a prefixed derived ID, matching the session ID used by
  file tools when they create file history.
- File change records now include a `tool_round_id` derived from the parent
  assistant tool-call round. `rewind` and TUI slash rollback can restore the
  latest tool round or a specified round ID, giving Phase 4 a practical
  whole-turn rollback path for multi-tool file changes.
- File-tool tests that intentionally exercise plain exact edit behavior now
  clear `PRIORITY_AGENT_SMART_EDIT` under the shared env guard, so the Phase 4
  `file_tool` gate is stable even when smart-edit tests run in parallel.

Validation so far:

- `cargo test -q file_tool` - passed, 50 tests.
- `cargo test -q file_state` - passed, 0 tests matched.
- `cargo test -q rewind_tool` - passed, 1 test.
- `cargo test -q rewind` - passed, 5 tests.
- `cargo test -q diff_tool` - passed, 3 tests.
- `cargo test -q diff` - passed, 29 tests.
- `cargo test -q checkpoint` - passed, 34 tests.
- `cargo test -q slash_handler` - passed, 49 tests.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.
- `cargo clippy -q -- -D warnings` - passed.
- `git diff --check` - passed.

## Phase 5: Permission, Hook, And Human Review Runtime

Purpose: make safety review one coherent product path.

Tasks:

1. Unify review requests for:
   - tool permission
   - destructive action
   - plan approval
   - model question to user
   - hook-originated question
   - subagent bubbled permission
   - remote/bridge action
2. Add durable review records:
   - input summary
   - risk facts
   - matched allow/deny/ask rules
   - classifier result
   - hook decision
   - user decision
   - persistence scope
   - saved config path
3. Expand hook lifecycle:
   - session start/end
   - user prompt submit
   - pre/post tool
   - post tool failure
   - permission request/resolution
   - file change
   - validation start/end
   - subagent start/end
   - pre/post compact
   - notification
4. Preserve env hooks as one provider while preparing project/user config.
5. Make denial recovery first-class:
   - safe fallback suggestion;
   - retry classification;
   - permission explanation command;
   - TUI review panel.

Primary files:

- `src/engine/human_review.rs`
- `src/engine/hooks.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/tui/slash_handler/permissions.rs`
- `src/tui/runtime_panels.rs`

Validation:

- `cargo test -q human_review`
- `cargo test -q permission_controller`
- `cargo test -q hooks`
- `cargo test -q recovery_plan`
- `cargo check -q`

Acceptance:

- Permission behavior is explainable without reading logs.
- Hooks can block, allow, annotate, or ask without bypassing explicit deny
  policy.

Progress on 2026-05-22:

- Added `HumanReviewAuditRecord` as a durable review snapshot for permission
  requests. It captures input summary, risk facts, matched rules, classifier
  output, user decision, persistence scope, saved config path, and recovery
  hints.
- Permission request records now embed the human review audit snapshot, and
  denied tool results return it in structured `permission_request` metadata.
- `PermissionRequested` and `PermissionResolved` trace events now carry the
  audit snapshot while keeping the existing prompt/decision fields compatible.
- `StreamEvent::PermissionRequest` can carry the same audit snapshot for UI and
  future panel rendering.
- Env hook provider now supports `PRIORITY_AGENT_PERMISSION_REQUEST_HOOK` and
  `PRIORITY_AGENT_PERMISSION_RESOLVED_HOOK`, records them in lifecycle
  snapshots, and lets permission-request hooks deny before the approval is
  shown to the user.
- Tool approval requests now carry the audit snapshot into the TUI. The
  approval panel renders risk facts, matched rules, and recovery hints so the
  review can be understood without opening logs.

Validation so far:

- `cargo test -q human_review` - passed, 8 tests.
- `cargo test -q permission_controller` - passed, 8 tests.
- `cargo test -q hooks` - passed, 9 tests.
- `cargo test -q trace` - passed, 37 tests.
- `cargo test -q recovery_plan` - passed, 13 tests.
- `cargo test -q runtime_panels` - passed, 5 tests.
- `cargo test -q main_screen` - passed, 15 tests.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.
- `cargo clippy -q -- -D warnings` - passed.
- `git diff --check` - passed.

## Phase 6: TUI Product Experience

Purpose: make the product feel stable instead of debug-heavy.

Priority surfaces:

1. Status line and `/status`
2. Tool progress and expandable output
3. Diff viewer
4. Permission approval panel
5. Task/terminal panel
6. Agent panel
7. Context/memory/skill panel
8. MCP/auth/repair panel
9. Trace/replay panel
10. Command palette and help maturity labels

Tasks:

1. Make all high-use panels read from `RuntimeAppState` selectors.
2. Add stable renderers for:
   - active tool rows;
   - completed/failed/denied tools;
   - file diff summaries;
   - pending approvals;
   - background tasks;
   - subagent tasks;
   - context pressure;
   - MCP health/auth state.
3. Hide or gate placeholder commands unless explicitly requested.
4. Add smoke/snapshot tests for the high-use surfaces.
5. Keep UI copy short and action-oriented.
6. Add `doctor` output that tells the user whether the install/runtime is
   product-ready.

Primary files:

- `src/tui/app.rs`
- `src/tui/commands.rs`
- `src/tui/runtime_panels.rs`
- `src/tui/tool_view.rs`
- `src/tui/screens/`
- `src/tui/components/`
- `src/tui/slash_handler/`

Validation:

- `cargo test -q commands`
- `cargo test -q runtime_panels`
- `cargo test -q status_bar`
- `cargo test -q tool_view`
- `cargo check -q`

Acceptance:

- During a real coding task, the user can see progress, risk, diffs, validation,
  and background work without opening raw trace logs.

Progress on 2026-05-22:

- The runtime approval panel now renders permission audit risk facts, matched
  rules, and recovery hints from the structured approval request instead of
  requiring trace-log inspection.
- Permission preview snapshot coverage was updated for command-scoped bash
  approval details.
- `/doctor` now starts with a product readiness summary. It reports `READY`,
  `USABLE_WITH_WARNINGS`, or `BLOCKED` from the current diagnostic report plus
  runtime selectors for failed tools, backgrounded tools, pending approvals, and
  MCP repair hints. JSON output also includes product readiness metadata and a
  `product_ready` check.
- Placeholder slash commands are now gated from default help and empty command
  palette results. They still appear when explicitly searched, and accepting one
  from the palette inserts the command instead of executing it immediately.
- `/panel` now includes stable `agents` and `trace` panels, and `/panel all`
  includes both. The agent panel summarizes profile definitions, running agents,
  durable task states, and recent artifacts; the trace panel summarizes latest
  and recent traces plus the replay status entrypoints.

Validation so far:

- `cargo test -q commands` - passed, 25 tests.
- `cargo test -q commands` - passed, 26 tests after placeholder gating.
- `cargo test -q command_palette` - passed, 5 tests.
- `cargo test -q help_maturity` - passed, 1 test.
- `cargo test -q runtime_panels` - passed, 5 tests.
- `cargo test -q runtime_panels` - passed, 7 tests after agent/trace panels.
- `cargo test -q status_bar` - passed, 3 tests.
- `cargo test -q tool_view` - passed, 12 tests.
- `cargo test -q main_screen` - passed, 15 tests.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.
- `cargo clippy -q -- -D warnings` - passed.
- `git diff --check` - passed.

## Phase 7: Subagents, Tasks, And Worktrees

Purpose: make parallel coding work dependable.

Tasks:

1. Harden agent definitions:
   - role
   - when to use
   - allowed/disallowed tools
   - MCP scope
   - permission mode
   - context mode
   - model policy
   - max turns
   - output contract
2. Support context modes:
   - minimal
   - inherited summary
   - full fork
   - isolated worktree fork
3. Make agent tasks durable:
   - status
   - transcript path
   - output file
   - in-progress tool ids
   - permission requests
   - worktree path/branch
   - cleanup action
4. Add background lifecycle:
   - launch
   - progress
   - wait/read output
   - cancel
   - merge/review/cleanup for worktree workers
5. Add result fusion only after artifacts are durable.
6. Add tests for worker failure, timeout, cancellation, merge conflict, and
   cleanup safety.

Primary files:

- `src/agent/`
- `src/tools/agent_tool/`
- `src/tools/task_tool/`
- `src/tools/worktree_tool/`
- `src/session_store/`
- `src/tui/slash_handler/agents.rs`

Validation:

- `cargo test -q agent_tool`
- `cargo test -q forked_context`
- `cargo test -q worktree_tool`
- `cargo test -q session_store`
- `cargo check -q`

Acceptance:

- Mutating parallel workers use isolated worktrees by default.
- Parent sessions can inspect, merge, or clean up worker output safely.

Progress on 2026-05-22:

- Code-change agent definitions now default to `isolated_worktree_fork` instead
  of `full_fork`, which makes the derived permission mode `isolated_write`.
- Built-in mutating profiles (`default` and `implementer`) now use isolated
  worktree context by default.
- The `agent` tool also infers `isolated_worktree_fork` when no profile is
  provided but the resolved tool surface includes mutating file tools such as
  `file_edit`, `file_write`, or `apply_patch`. Explicit `context_mode` still
  overrides the inference.
- Durable subagent task state now records post-launch wait failures as
  `failed` or `timed_out` instead of leaving spawned workers stuck in `running`.
  The failure state preserves the original payload, including isolated worktree
  cleanup metadata.
- The `agent` tool now supports `agent_id` with `action=cancel`, which sends the
  manager stop signal and marks durable task state `cancelled` while preserving
  permission requests and worktree cleanup metadata.
- Agent worktree safety guards now have direct coverage for dirty status
  detection, untracked path detection, safe `codex/agent-*` branch deletion, and
  rejection of non-isolated task records.
- The `agent` tool now supports `agent_id` with `action=read`, which reads
  durable task state and persisted result artifacts from the current session
  store. This lets a parent inspect worker output after the in-memory manager
  result is gone.

Validation so far:

- `cargo test -q profiles` - passed, 6 tests.
- `cargo test -q agent_tool` - passed, 14 tests.
- `cargo test -q agent_tool` - passed, 15 tests after failure-state handling.
- `cargo test -q agent_tool` - passed, 17 tests after cancel action support.
- `cargo test -q agent_tool` - passed, 18 tests after durable read support.
- `cargo test -q forked_context` - passed, 4 tests.
- `cargo test -q worktree_tool` - passed, 3 tests.
- `cargo test -q worktree_tool` - passed, 6 tests after safety guard coverage.
- `cargo test -q session_store` - passed, 14 tests.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.

## Phase 8: Context, Compaction, Memory, And Skills

Purpose: keep long coding sessions coherent.

Tasks:

1. Treat compaction as runtime state:
   - token pressure
   - trigger
   - strategy
   - preserved messages
   - compact boundary id
   - retained tools/memory/skills
2. Support strategies:
   - no-op
   - snip/read-search collapse
   - microcompact
   - auto compact
   - reactive compact after context errors
   - session-memory compact
3. Add background memory extraction with strict quality gates.
4. Keep project memory, nested memory, and skill triggers explicit.
5. Add skill discovery/activation evidence without bloating prompts.
6. Make `/context`, `/memory`, and `/skills` explain inclusion reasons.

Primary files:

- `src/engine/context_compressor.rs`
- `src/engine/context_collapse.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/memory_sync_controller.rs`
- `src/memory/`
- `src/skills/`

Validation:

- `cargo test -q context_compressor`
- `cargo test -q context_collapse`
- `cargo test -q memory_sync_controller`
- `cargo test -q retained_context`
- `cargo check -q`

Acceptance:

- A long task survives compaction without losing the active objective, changed
  files, validation state, or active subagent/task state.

## Phase 9: MCP, Plugins, Bridge, Remote, And Providers

Purpose: productize integrations after the local programming harness is stable.

Tasks:

1. MCP:
   - tools/resources/prompts/commands as runtime facts;
   - auth and approval repair;
   - per-agent server scopes;
   - reconnect and health status.
2. Plugins:
   - install/enable/disable/reload;
   - trust/signature policy;
   - tool/command/skill contributions;
   - diagnostic errors.
3. Bridge/remote:
   - remote state in runtime state;
   - permission callbacks;
   - replay cursor;
   - reconnect;
   - status panel.
4. Providers:
   - capability records;
   - normalized message/tool-result conversion;
   - typed recovery for protocol/context/rate/auth failures;
   - fallback model policy;
   - context-too-long reactive compaction.

Primary files:

- `src/engine/mcp.rs`
- `src/tools/mcp_tool/`
- `src/plugins/`
- `src/bridge/`
- `src/remote/`
- `src/services/api/`
- `src/engine/error_classifier.rs`

Validation:

- `cargo test -q mcp`
- `cargo test -q mcp_tool`
- `cargo test -q plugins`
- `cargo test -q provider_protocol`
- `cargo check --features experimental-api-server -q`
- `cargo check -q`

Acceptance:

- Integration failures are visible, diagnosable, and recoverable from the CLI.

## Phase 10: Release Hardening

Purpose: make the project shippable, not only impressive locally.

Tasks:

1. Install/update:
   - fast install path;
   - clear dependency checks;
   - rollback on failed update;
   - version reporting.
2. Doctor:
   - provider config;
   - model/tool-call support;
   - shell backend;
   - permissions config;
   - MCP/plugin status;
   - writable state directories;
   - git/worktree availability.
3. Configuration:
   - stable config schema;
   - migration checks;
   - user/project/global scope;
   - redacted export.
4. Packaging:
   - binary release process;
   - checksums;
   - install script smoke test;
   - release notes.
5. CI/release gates:
   - `cargo fmt --check`
   - `cargo check -q`
   - `cargo test -q`
   - `cargo clippy --all-features -- -D warnings`
   - feature checks
   - replay matrix
   - external parity report import validation
6. Documentation:
   - quick start;
   - safety model;
   - tool permissions;
   - common workflows;
   - debugging/doctor guide;
   - known gaps.

Primary files:

- `install.sh`
- `Cargo.toml`
- `scripts/`
- `.github/workflows/`
- `docs/`
- `src/tui/slash_handler/diagnostics.rs`

Acceptance:

- A fresh user can install, configure, run a code task, inspect permissions, and
  recover from common failures without reading source code.

## Immediate Development Queue

The next concrete batches should be:

1. **Ordered tool orchestration.**
   Fix mixed read/write tool ordering and add regression tests.
2. **Real external baseline capture.**
   Run Claude Code and Codex CLI on the six parity scenarios, import, validate,
   and record parity reports.
3. **Bash safety and task-output hardening.**
   Deepen command facts, durable output, auto-background, and permission review.
4. **File history and rewind hardening.**
   Add turn-level snapshots, restore coverage, and closeout evidence plumbing.
5. **TUI product pass for coding loop.**
   Polish status, tool output, diff, approval, tasks, agents, and context panels.
6. **Subagent lifecycle hardening.**
   Make worker progress, output, cancellation, merge, and cleanup reliable.
7. **Context/memory long-session pass.**
   Prove compaction and memory retention on long real coding tasks.
8. **Release hardening.**
   Install/update/doctor/config/docs/CI.

Do not start broad UI or release packaging before ordered tool execution and
real external baseline evidence exist. Those two items are the fastest way to
separate real programming gaps from guessed gaps.

## Verification Policy

Use the narrowest validation gate for each batch, then broaden when shared
contracts move.

Common targeted gates:

```bash
cargo test -q tool_execution
cargo test -q command_classifier
cargo test -q file_tool
cargo test -q runtime_state
cargo test -q human_review
cargo test -q hooks
cargo test -q agent_tool
cargo test -q context_compressor
cargo test -q provider_protocol
cargo check -q
```

Broader gates before marking a phase complete:

```bash
cargo fmt --check
cargo test -q
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
bash scripts/workflow-production-gates.sh
git diff --check
```

External parity gates:

```text
/eval baseline-import <artifact_path> claude-code <model>
/eval baseline-import <artifact_path> codex <model>
/eval baseline-validate all
/eval parity all
/eval parity-record all
```

## Decision Log

- The project should pursue Claude Code-like product completeness for
  programming tasks before heavy differentiation.
- Priority Agent should still remain narrow, deep, personal, and verifiable;
  personalization comes after the mainstream coding-agent harness is reliable.
- The most urgent implementation gap is ordered mixed tool-call execution.
- Real Claude Code/Codex baseline artifacts should be collected early, not
  postponed until after every planned improvement.
- Parity must be proven through runtime behavior and evidence, not tool count.

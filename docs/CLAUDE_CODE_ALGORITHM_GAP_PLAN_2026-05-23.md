# Claude Code Algorithm Gap Plan

Date: 2026-05-23

Status: created after local code review and Claude source comparison.

Goal: close the remaining coding-agent maturity gap at the concrete algorithm
level: tool execution, file mutation, bash permissioning, permission state,
compaction continuity, and release dogfood feedback loops.

This plan is Claude Code-informed. It must not copy Claude source bodies,
vendor prompt text, analytics names, or UI strings. The target is to reproduce
the behavior shape with Priority Agent's Rust and desktop runtime types.

## Review Fixes Landed In This Pass

1. `scripts/run_live_eval.sh` summary mode now really requires `--run-id`.
   Previously the script generated a default run id before the summary-mode
   check, so the documented guard was ineffective.
2. `core-rust-multi-file-refactor` now actually forces a multi-file Rust
   repair: `report.rs` must call `stats::average`, and `stats.rs` contains the
   seeded bad helper.
3. `file_edit` now defaults to Claude-like read-before-edit discipline for
   existing files. The old behavior was only enabled with
   `PRIORITY_AGENT_SMART_EDIT=1`, which allowed silent edits without session
   read state. A narrow rollback switch remains:
   `PRIORITY_AGENT_ALLOW_EDIT_WITHOUT_READ=1`.

## Claude Source Inspected

- Tool contract and registry:
  `/Users/georgexu/Desktop/claude/src/Tool.ts`,
  `/Users/georgexu/Desktop/claude/src/tools.ts`
- File editing and history:
  `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/FileEditTool.ts`,
  `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts`,
  `/Users/georgexu/Desktop/claude/src/utils/settings/validateEditTool.ts`
- Bash and shell permissions:
  `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`,
  `/Users/georgexu/Desktop/claude/src/tools/BashTool/bashPermissions.ts`,
  `/Users/georgexu/Desktop/claude/src/tools/BashTool/readOnlyValidation.ts`,
  `/Users/georgexu/Desktop/claude/src/tools/BashTool/sedEditParser.ts`
- Permission state machine:
  `/Users/georgexu/Desktop/claude/src/utils/permissions/permissions.ts`
- Query, tool result, abort, and compaction:
  `/Users/georgexu/Desktop/claude/src/query.ts`,
  `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`,
  `/Users/georgexu/Desktop/claude/src/services/compact/`

Priority Agent source anchors:

- `src/tools/mod.rs`
- `src/tools/file_tool/`
- `src/tools/bash_tool/`
- `src/permissions/mod.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/engine/context_compressor.rs`
- `src/engine/streaming.rs`

## Remaining Algorithm Gaps

### Track 1: Bash Permission AST And Redirection Safety

Claude's bash path is not just command keyword classification. It uses a
security pipeline that prefers AST-derived subcommands and redirects, caps
legacy subcommand fanout, handles `cd` specially, validates original command
redirections after subcommand rules, and merges permission suggestions so a
compound command prompt does not hide a risky second action.

Priority Agent has useful structured bash facts now, but the parser is still
heuristic. The next implementation should add a conservative shell parse layer:

1. Produce `BashCommandPlan` with parser status, subcommand spans, redirects,
   cd commands, git commands, process substitutions, command substitutions, and
   heredoc facts.
2. Fail closed to permission ask when parsing is unavailable, too complex, or
   structurally ambiguous.
3. Apply permission order as a tested state machine:
   deny subcommands first, original redirection path checks second, ask merging
   third, allow only when all subcommands are safe and no injection path remains.
4. Add replay tests for:
   - `cd outside && git status`
   - `echo ok >> /outside/file`
   - `cd outside && python3 script.py`
   - heredoc write attempts
   - `sed -i`, `perl -pi`, `python -c open(..., "w")`, `tee`
   - more than the supported subcommand cap

Acceptance:

```bash
cargo test -q command_classifier -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo test -q permission_controller -- --test-threads=1
scripts/release-dogfood-gate.sh quick
```

### Track 2: Permission Mode And Auto-Decision State Machine

Claude's permission flow has distinct phases for rule decisions, promptless
contexts, accept-edits fast paths, auto classifiers, fail-closed classifier
unavailability, and denial tracking. Priority Agent has rule-based modes and
structured evidence, but it does not yet have the same explicit state machine
for headless/background work or repeated denial recovery.

Next implementation:

1. Define a `PermissionPipelineStage` enum and record each stage in
   `permission_decision_evidence.v1`.
2. Add promptless/headless behavior: if a tool requires interactive approval and
   no prompt can be shown, deny with recovery rather than falling into a vague
   ask state.
3. Add denial tracking per session/tool family so repeated denied risky actions
   produce bounded recovery instead of repeated prompts.
4. Keep auto-allow paths narrow: file edits in workspace may fast-path only
   after file validation passes; shell mutation never bypasses shell safety
   evidence.

Acceptance:

```bash
cargo test -q permissions -- --test-threads=1
cargo test -q human_review -- --test-threads=1
cargo test -q permission_controller -- --test-threads=1
```

### Track 3: File Mutation Fail-Closed History And Settings Validation

The file edit path is closer now: read-before-edit is default, stale checks and
diff metadata exist, and text encoding/line ending preservation is tested. The
remaining gap is fail-closed mutation history and settings-aware validation.

Next implementation:

1. Refuse `file_edit` if checkpoint/history creation fails before the write.
   `file_patch` already does this more strictly; single-file edit should match.
2. Add settings schema validation for Priority Agent config files before writes:
   `.priority-agent/*.toml`, provider settings, permission settings, and desktop
   app settings paths.
3. Split validation stages into durable data:
   path guard, read-state guard, stale guard, match guard, schema guard,
   checkpoint guard, pre-write race guard, write, diagnostics.
4. Add rollback smoke tests that restore a successful `file_edit` and prove the
   file change record is sufficient for desktop trace/debug.

Acceptance:

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q file_patch -- --test-threads=1
cargo test -q diagnostics -- --test-threads=1
```

### Track 4: Tool Execution Abort And Result Pair Completeness

Claude carefully handles aborts before streaming, during streaming, and during
tool execution so tool-use blocks do not become orphaned without tool-result
blocks. Priority Agent has ordered execution and lifecycle metadata, but abort
and interruption evidence still needs stronger invariants.

Next implementation:

1. Add tests for abort during:
   - queued read-only tools,
   - running read-only batch,
   - running mutating tool,
   - permission prompt,
   - background terminal task start.
2. Guarantee every provider-visible tool call receives a terminal result:
   success, error, permission denied, not exposed, interrupted, or cancelled.
3. Surface interruption recovery in timeline and trace, not only logs.
4. Add lifecycle assertions to `ToolExecutionBatch` so order and result-pair
   completeness cannot regress.

Acceptance:

```bash
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q streaming -- --test-threads=1
cargo test -q trace -- --test-threads=1
```

### Track 5: Compaction Order And Post-Compact Restoration

Claude uses a layered compaction pipeline: snip, microcompact, context
collapse/projection, autocompact, reactive compact retry, then post-compact
restoration of important attachments. Priority Agent now preserves runtime
continuity facts, but the algorithm is still simpler and mostly heuristic.

Next implementation:

1. Make compaction order explicit in code and trace:
   snip old tool results, microcompact tool output, summarize, then restore
   critical files/context.
2. Preserve recently read files and invoked skills as post-compact attachments
   when they are needed for continued editing.
3. Add reactive retry for prompt-too-long/provider context errors before giving
   up the turn.
4. Track compaction token deltas and retained items so release dogfood can say
   whether compaction helped or damaged the run.

Acceptance:

```bash
cargo test -q context_compressor -- --test-threads=1
cargo test -q runtime_continuity -- --test-threads=1
cargo test -q prompt_context -- --test-threads=1
```

### Track 6: Release Dogfood Feedback Loop

The six-scenario release gate now exists. The next step is not to add more
cases immediately; it is to make failures feed back into product work with
stable ownership and evidence.

Next implementation:

1. Run:

```bash
scripts/release-dogfood-gate.sh agent-run --run-tests --timeout 2400
```

2. Generate summary:

```bash
scripts/release-dogfood-gate.sh summary --run-id <run-id>
```

3. Add a small parser/report that classifies each miss as:
   - tool_contract
   - file_state
   - bash_permission
   - permission_recovery
   - compaction_continuity
   - llm_reasoning
   - desktop_evidence
4. Use that report to choose the next implementation batch. Do not expand the
   release suite until the current six cases are stable and explainable.

## Suggested Execution Order

1. Track 1: bash AST/redirection permission safety.
2. Track 3: file mutation checkpoint fail-closed and settings validation.
3. Track 4: abort/result-pair completeness.
4. Track 2: permission mode state machine and denial tracking.
5. Track 5: compaction ordering and restoration.
6. Track 6: run full release dogfood, then iterate from the failure report.

The first two tracks are the highest leverage for real coding quality because
they directly protect writes and shell mutations.

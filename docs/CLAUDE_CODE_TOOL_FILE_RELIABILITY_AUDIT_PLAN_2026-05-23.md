# Claude Code Tool And File Reliability Audit Plan

Date: 2026-05-23

Status: focused follow-up plan for closing the programming-quality gap before
more desktop UI polish.

Goal: make Priority Agent reliable enough for daily coding work by deepening
tool contracts, file editing, bash execution, permission evidence, recovery,
and desktop-visible runtime facts. This plan is Claude Code-informed, but all
implementation must stay original Rust/TypeScript owned by this repository.

## Why This Is The Next Work

The macOS desktop app now has enough architecture to become the primary product
surface. The remaining gap that matters most for real usage is not another
visual pass. It is whether the agent can safely read, edit, validate, recover,
and explain its work in a real repository.

Claude Code is still stronger in three areas:

1. Tool calls are product-level state, not just JSON executor calls.
2. File edits are guarded by a careful validation, permission, history, diff,
   diagnostic, and rollback sequence.
3. Bash commands are classified, permissioned, surfaced, and recovered as
   terminal tasks rather than raw shell output.

Priority Agent already has the right foundations: ordered tool orchestration,
Tool Contract V2 metadata, file state tracking, checkpoints, bash command
classification, permission audit records, runtime state, trace, and desktop
timeline cards. The next phase is to audit the weak edges and turn those
foundations into release gates.

## Claude Reference Scope

Before each batch, re-check the local Claude source for behavior shape:

- Tool contract: `/Users/georgexu/Desktop/claude/src/Tool.ts`
- Tool registry: `/Users/georgexu/Desktop/claude/src/tools.ts`
- Tool execution services:
  `/Users/georgexu/Desktop/claude/src/services/tools/`
- Query loop and state:
  `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`
  and `/Users/georgexu/Desktop/claude/src/query.ts`
- File editing:
  `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/FileEditTool.ts`
- File history: `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts`
- Bash:
  `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`
- Permissions:
  `/Users/georgexu/Desktop/claude/src/utils/permissions/permissions.ts`
- Context compaction:
  `/Users/georgexu/Desktop/claude/src/services/compact/`

Use these as semantic references only. Do not copy implementation bodies,
prompt strings, vendor feature flags, analytics names, or UI copy.

## Priority Agent Anchors

Primary runtime files:

- `src/tools/mod.rs`
- `src/tools/file_tool/`
- `src/tools/bash_tool/`
- `src/permissions/mod.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/engine/evidence_ledger.rs`
- `src/engine/streaming.rs`
- `src/state/runtime_state.rs`
- `src/desktop_runtime/`
- `apps/desktop/src/app/`

Existing strengths to preserve:

- ordered mixed read/write tool execution;
- rich `Tool` trait metadata hooks;
- file read-state tracking, stale detection, checkpoint history, and diff
  records;
- bash command facts and command-scoped permission rules;
- human-review audit records and trace events;
- desktop timeline, trace drawer, context drawer, and permission cards.

## Plan Audit Result

The first version of this plan had the right direction, but it was still too
much of a capability checklist. To close the Claude Code gap, implementation
must mirror Claude's algorithmic shape more explicitly:

- preserve the same guard ordering before a tool mutates state;
- preserve the same state-machine transitions for tool execution, permission,
  file edits, bash background tasks, and compaction;
- preserve the same fail-closed defaults for uncertain tools and uncertain
  shell parsing;
- preserve the split between model-facing result payloads, UI-facing render
  facts, trace/debug facts, and durable recovery state;
- preserve error branches as product behavior, not only as logged failures.

The code must not copy Claude source bodies or text. But every batch below
should start by extracting the behavior from the referenced Claude files, then
implementing that behavior with Priority Agent's Rust and desktop types.

## Claude Algorithm Replication Matrix

| Area | Claude behavior to mirror | Priority Agent implementation target |
|------|---------------------------|--------------------------------------|
| Tool construction | `buildTool` fills complete tool objects with fail-closed defaults: not concurrency-safe, not read-only, not destructive, classifier input empty, permission falls through to the general permission layer. | Add an audit profile that records defaulted security-relevant methods. Release-visible tools must explicitly override read/write/concurrency/permission/classifier behavior when relevant. |
| Tool contract | A tool owns schema, output schema, validate, permission, permission matcher, classifier input, UI summary, activity text, result mapping, max result size, interrupt behavior, deferred visibility, open-world and user-interaction flags. | Treat `Tool` metadata as the source of truth for runtime, permission, desktop timeline, trace, provider payload, and closeout. Do not let UI or closeout infer semantics from free-form strings. |
| Tool execution | Consecutive concurrency-safe calls can run together; mutating or unsafe tools form serial barriers. Streaming execution keeps non-concurrent tools exclusive, uses child abort controllers, and handles sibling failure/interruption deliberately. | Keep ordered partitioning as a release invariant. Add audit tests that prove every tool's concurrency flags are intentional and that cancellation/interrupt behavior is enforced. |
| Permission precedence | Deny rules win first; ask rules can force review; tool-specific checks run before broad allow/bypass; content-specific ask and safety checks remain bypass-immune; passthrough converts to ask. | Normalize permission stages into `PermissionDecisionEvidence` with stage, rule, risk, classifier, hook, mode, and subcommand/path facts. Tests should assert precedence, not only final allow/deny. |
| File edit validation | Expand/backfill path, reject secrets and no-op edits, enforce deny rules, avoid UNC fs probes, cap file size, detect encoding and line endings, validate create vs edit, reject notebook wrong tool, require full read state, stale-check by mtime plus content fallback, find actual old string, reject ambiguous matches, validate settings edits. | Implement one explicit file-edit validation pipeline and expose the stage that failed. Add tests for each branch and ensure desktop cards render the branch-specific recovery. |
| File edit write path | Prepare parent directory and history backup before the critical section; then perform read-stale-check-patch-write without yielding between stale check and write; preserve encoding/line endings/quote style; update read state; notify diagnostics; return structured patch/diff data. | Hold the per-file mutation lock across stale check and write. Return structured edit preview plus checkpoint/file-change ids. Preserve text format and diagnostics metadata. |
| Bash read/search detection | Bash read-only status depends on command parsing, `cd` handling, and search/read/list classification; uncertain parsing is conservative. | Store command facts per subcommand and use them for read-only/concurrency, permission, timeline, and validation evidence. |
| Bash permission | Parse for security once; prefer AST-derived subcommands; fall back carefully; cap subcommand fanout; deny if any subcommand denies; ask if any asks; check original redirections because split output may hide them; only suggest exact/prefix rules when scoped enough. | Build a `BashPermissionEvidence` model with parser status, subcommands, redirections, deny/ask/allow reason, and exact/prefix suggestion. Unknown or too-complex commands must ask. |
| Bash task lifecycle | Long/blocking commands can be backgrounded, output is persisted when large, model-facing output receives a preview/path wrapper, UI displays raw task state, and background/cancel/read-output paths use task ids. | Keep terminal task state durable and desktop-visible. Separate model result text from UI task cards and trace facts. |
| Query and compaction | Tool output size handling happens before microcompact; snip/microcompact/autocompact/reactive compact have ordered recovery paths; compact boundaries are persisted and surfaced; abort checks happen before and after streaming/tool calls. | Treat compaction as a runtime state transition with traceable boundary metadata. Preserve changed files, validation, permissions, background tasks, and context attachments across compaction. |

## Per-Batch Extraction Checklist

Every implementation batch in this plan should include a short design note in
the commit message, PR description, or progress log:

```text
Claude source inspected:
- files/functions:
Algorithm extracted:
- guard order:
- state carried:
- concurrency/abort boundary:
- permission checkpoints:
- error branches:
- model-facing result:
- UI/trace/recovery facts:
Priority Agent implementation:
- Rust/TS files:
- new structs/enums:
- deliberate divergences:
Tests:
- branch coverage:
- regression fixtures:
- validation commands:
```

If the extracted algorithm cannot be implemented directly, record the
divergence before coding. Silent divergence is a release risk.

## Non-Goals

- Do not pause this work for another broad desktop visual polish pass.
- Do not replace the runtime with the experimental HTTP API path.
- Do not add prompt-heavy behavioral rules when a tool contract, runtime check,
  or test can enforce the behavior.
- Do not chase every Claude feature noun. The target is reliable local coding,
  not feature-count parity.
- Do not claim parity until the release gates below pass.

## Reliability Invariants

These invariants should become tests or debug assertions where practical:

1. A write tool must never rely on display-only read output as source content.
2. A mutation must pass validation, permission, stale-read checks, and history
   preparation before bytes are changed.
3. Every successful mutation must produce structured before/after evidence and
   a usable rollback handle.
4. Bash must not silently bypass file-edit safety through shell wrappers,
   heredocs, `sed -i`, `python -c`, `tee`, or similar mutation paths.
5. Permission cards and trace records must say why a tool was allowed, denied,
   auto-allowed, or escalated.
6. Long or large tool output must be summarized for the model and preserved as
   an artifact when it matters.
7. Desktop UI must render high-value runtime facts directly; trace is for
   debugging, not the only way to understand what happened.

## Track A: Tool Contract Reliability Audit

Priority: P0. This is the first implementation batch.

Purpose: ensure every release-visible tool has explicit, audited semantics
instead of accidentally inheriting broad defaults.

Tasks:

1. Add a registry audit that enumerates all registered tools and produces a
   `ToolReliabilityProfile` for representative inputs.
2. Classify each tool by:
   - operation kind;
   - read-only status;
   - concurrency safety;
   - destructive status;
   - open-world behavior;
   - user-interaction requirement;
   - interrupt behavior;
   - max result size;
   - permission matcher input;
   - input paths;
   - transcript summary;
   - UI render kind;
   - provider payload/output schema readiness.
3. Track which security-relevant methods are inherited from trait defaults.
   Claude's `buildTool` defaults fail closed for concurrency and read-only; our
   audit should make default usage visible and fail release-visible tools that
   accidentally rely on broad defaults.
4. Add representative input fixtures per high-risk tool so the audit can test
   parameter-dependent semantics, not only schema-level metadata:
   - read-only bash command;
   - mutating bash command;
   - file read;
   - file edit;
   - file patch;
   - destructive git or shell command;
   - external/open-world network command;
   - background task command.
5. Add a test gate that fails for release-visible tools missing critical
   metadata. Start strict for `bash`, `file_read`, `file_write`, `file_edit`,
   `file_patch`, `grep`, `glob`, `agent`, task tools, web/MCP tools, worktree,
   and remote/dev tools.
6. Emit the audit snapshot through `/tool-audit` or an internal diagnostics
   path so the desktop app and future release gate can read it.
7. Document intentional exceptions near the tool implementation, not only in
   this plan.

Target files:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/tui/commands.rs`
- `src/diagnostics/` or a small new runtime diagnostics module

Validation:

```bash
cargo test -q tool_contract
cargo test -q tool_metadata
cargo test -q tool_reliability
cargo check -q
```

Acceptance:

- The audit clearly shows which tools can read, mutate, run concurrently,
  require permission, and survive interruption.
- A newly added release-visible tool cannot silently ship with unsafe default
  semantics.

## Track B: File Edit Reliability And Recovery

Priority: P0 after Track A.

Purpose: make file mutation Claude-like at the behavior level: precise,
recoverable, explainable, and hard to corrupt.

Tasks:

1. Return a structured edit preview for `file_edit`, `file_write`, and
   `file_patch`:
   - canonical path and display path;
   - read coverage used;
   - before/after hash;
   - encoding and line-ending mode;
   - changed ranges;
   - additions/deletions;
   - bounded diff preview;
   - checkpoint id and file change id;
   - external-modification status;
   - rollback handle.
2. Make the validation pipeline explicit and ordered after Claude's
   `FileEditTool` behavior:
   - path expansion and observable input backfill;
   - secret/high-risk target guard;
   - no-op edit rejection;
   - deny-rule check before fs mutation;
   - UNC/network path safety handling before fs probes;
   - size cap;
   - byte read with encoding and line-ending detection;
   - nonexistent-file create vs edit distinction;
   - wrong-tool rejection for notebooks or binary-like files;
   - full/relevant read-state requirement;
   - stale mtime check with content-equality fallback;
   - actual old-string discovery;
   - multiple-match rejection unless replace-all is explicit;
   - settings/config validation before write.
3. Preserve Claude's write-path shape:
   - prepare parent directory and history backup before the critical section;
   - hold the per-file mutation lock across stale check and write;
   - avoid async/yield points between stale check and write;
   - preserve encoding, line endings, and quote style;
   - update file read state after successful write;
   - notify diagnostics/LSP when available;
   - return structured patch/diff data once.
4. Strengthen exact replacement diagnostics:
   - reject `old_string == new_string`;
   - detect zero matches with nearby context suggestions;
   - detect multiple matches and require explicit replace-all or a narrower
     anchor;
   - explain partial-read conflicts.
5. Add a conflict flow when a file changed after read:
   - no silent overwrite;
   - show old read hash and current hash;
   - suggest re-read or patch regeneration.
6. Add guardrails for high-risk targets:
   - secrets and env files;
   - home/config paths outside the selected project;
   - generated/build artifacts;
   - very large files;
   - binary/non-text files.
7. Integrate diagnostics where available:
   - capture before/after LSP diagnostics for touched files;
   - surface formatter-caused drift separately from agent edits.
8. Make rollback first-class in desktop:
   - file card can expose rollback id;
   - trace can show checkpoint metadata;
   - future button can call the same runtime path as `/rewind`.

Target files:

- `src/tools/file_tool/mod.rs`
- `src/tools/file_tool/patch.rs`
- `src/tools/file_tool/history.rs`
- `src/tools/file_tool/diagnostics.rs`
- `src/engine/checkpoint.rs`
- `src/desktop_runtime/`
- `apps/desktop/src/app/components/timeline/`

Validation:

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q file_patch -- --test-threads=1
cargo test -q checkpoint
cargo test -q rewind
cargo test -q diff
cargo check -q
```

Acceptance:

- Failed edits tell the model and user exactly how to recover.
- Successful edits produce enough structured evidence for closeout, desktop
  timeline, trace, and rollback without parsing free-form text.

## Track C: Bash Reliability, Safety, And Terminal Tasks

Priority: P1.

Purpose: keep shell useful while preventing shell from becoming an unsafe file
mutation escape hatch.

Tasks:

1. Deepen command semantic parsing for compound commands:
   - keep per-subcommand facts;
   - cap expensive analysis;
   - conservatively ask when the parser cannot prove safety.
2. Mirror Claude's permission algorithm shape for bash:
   - parse once for security and reuse parser facts;
   - prefer AST-derived subcommands when available;
   - fall back to legacy splitting only with divergence checks;
   - cap subcommand fanout and ask when the cap is exceeded;
   - check `cd`/working-directory and git/path constraints before broad allow;
   - check output redirections against the original command, not only split
     subcommands;
   - deny the whole command if any subcommand denies;
   - ask if any subcommand asks;
   - only allow compound commands when every subcommand is allowed or proven
     safe;
   - produce exact/prefix permission suggestions only when scoped enough.
3. Add explicit mutation detection for common shell edit paths:
   - `sed -i`;
   - `perl -pi`;
   - `python -c` / `python <<`;
   - `cat >`, `tee`, heredoc writes;
   - `apply_patch`;
   - shell redirection into project files.
4. Route detected file mutation attempts toward file tools when possible.
   If bash remains necessary, require permission with a clear reason and record
   changed-path evidence.
5. Make terminal tasks more visible:
   - background handle;
   - output artifact path;
   - stdout/stderr byte counts;
   - silent-success flag;
   - cancel status;
   - validation/test family;
   - dev-server/watch classification.
6. Separate model-facing shell output from UI-facing task state, matching
   Claude's split between result mapping and UI rendering:
   - the model receives bounded output or a persisted-output preview/path;
   - the desktop UI receives raw task status, output availability, and
     cancellation state;
   - trace receives command facts, parser facts, permission facts, and output
     artifact metadata.
7. Rename or explain soft sandbox behavior in user-facing surfaces so it is not
   mistaken for OS-level isolation.
8. Add recovery messages for:
   - command not found;
   - missing dev tools;
   - permission denied;
   - interactive command needs PTY;
   - long-running command moved to background;
   - non-zero validation command.

Target files:

- `src/tools/bash_tool/`
- `src/tools/bash_tool/command_classifier.rs`
- `src/task_manager/`
- `src/permissions/mod.rs`
- `src/state/runtime_state.rs`
- `apps/desktop/src/app/components/timeline/`

Validation:

```bash
cargo test -q command_classifier
cargo test -q bash_tool -- --test-threads=1
cargo test -q permissions
cargo test -q runtime_state
cargo check -q
```

Acceptance:

- Bash cards distinguish read/search, validation, package/network, file
  mutation, git mutation, destructive, and interactive commands.
- Shell-based mutations cannot bypass the file-edit evidence path silently.

## Track D: Permission Evidence And Denial Recovery

Priority: P1.

Purpose: make permission decisions explainable and recoverable in CLI, TUI, and
desktop.

Tasks:

1. Normalize a `PermissionDecisionEvidence` object:
   - permission mode;
   - matched allow/deny/ask rules;
   - risk level;
   - risk facts;
   - classifier result;
   - subcommand facts;
   - path facts;
   - hook decision;
   - user decision;
   - persisted scope and config path.
2. Preserve Claude's permission precedence as tests:
   - explicit deny rule wins before tool-specific checks;
   - explicit ask rule forces review unless a proven sandbox auto-allow applies;
   - tool-specific deny wins;
   - tool-specific content ask and safety checks are bypass-immune;
   - bypass/auto mode applies only after deny, content ask, and safety checks;
   - always-allow rules apply after bypass checks;
   - passthrough becomes ask.
3. Attach this evidence to:
   - permission request stream event;
   - denied tool result;
   - evidence ledger;
   - trace drawer;
   - desktop timeline card.
4. Track repeated denials and recoveries:
   - recommend safer tool path;
   - avoid retrying the same denied tool loop;
   - suggest rule edits only when scope is specific enough.
5. Prepare a Settings permission-rule editor later, but keep this batch runtime
   first.

Target files:

- `src/engine/human_review.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/permissions/mod.rs`
- `src/engine/evidence_ledger.rs`
- `src/desktop_runtime/events.rs`
- `apps/desktop/src/app/components/timeline/`

Validation:

```bash
cargo test -q human_review
cargo test -q permission_controller
cargo test -q permissions
cargo test -q trace
cargo check -q
```

Acceptance:

- A user can understand why a permission prompt appeared without reading logs.
- The model gets a structured denial recovery path instead of vague failure
  text.

## Track E: Context, Output, And Compaction Continuity

Priority: P2.

Purpose: long coding sessions should not lose the active task, changed files,
validation evidence, or background task output.

Tasks:

1. Define compactable runtime facts:
   - current objective;
   - active session/project;
   - recently read files;
   - changed files and checkpoint ids;
   - validation commands and outcomes;
   - pending permissions;
   - background tasks;
   - context attachments such as current diff.
2. Mirror Claude's compaction ordering at the behavior level:
   - cap or persist oversized tool results before compaction;
   - apply cheap/surgical collapse before heavier summarization when possible;
   - run microcompact before autocompact when both are available;
   - record compact boundaries as session state;
   - on provider context errors, try reactive compaction once with loop guards;
   - preserve abort checks before and after streaming/tool calls.
3. Ensure large tool results are replaced with high-signal previews plus
   artifact references.
4. Add reactive compaction recovery when providers reject context length.
5. Show compaction boundaries in trace and desktop timeline.

Target files:

- `src/engine/context_compressor.rs`
- `src/engine/retrieval_context.rs`
- `src/engine/evidence_ledger.rs`
- `src/session_store/`
- `apps/desktop/src/app/components/trace/`

Validation:

```bash
cargo test -q context_compressor
cargo test -q retrieval_context
cargo test -q evidence_ledger
cargo check -q
```

Acceptance:

- After compaction, the agent still knows what it changed, what tests ran, and
  what remains to validate.

## Track F: Desktop Runtime Integration

Priority: P2 after Tracks A-D have structured data.

Purpose: make the desktop app show reliability facts directly, so it becomes a
better coding product than a prettier CLI wrapper.

Tasks:

1. Map structured file edit previews into timeline cards:
   - changed files;
   - diff preview;
   - conflict state;
   - diagnostics;
   - rollback id.
2. Keep model-facing and UI-facing payloads separate:
   - model payload stays concise and bounded;
   - UI payload carries rich structured cards;
   - trace payload carries full diagnostic facts;
   - durable state carries recovery ids and artifact paths.
3. Map bash facts into terminal-task cards:
   - command category;
   - validation status;
   - background handle;
   - output artifact;
   - failure reason and recovery.
4. Map permission evidence into approval cards:
   - why this needs review;
   - what scope is being approved;
   - deny/retry guidance.
5. Expose the same facts in trace drawer/debug export.

Target files:

- `src/desktop_runtime/events.rs`
- `apps/desktop/src/app/runtime/`
- `apps/desktop/src/app/components/timeline/`
- `apps/desktop/src/app/components/trace/`
- `apps/desktop/src/app/runEventState.ts`

Validation:

```bash
corepack pnpm --dir apps/desktop build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
corepack pnpm --dir apps/desktop test:ui-smoke
```

Acceptance:

- A real coding run can be understood from the main transcript without opening
  raw logs.
- Trace remains detailed enough to debug bad runtime decisions.

## Track G: Release Reliability Gauntlet

Priority: P0 as a gate, P2 for full automation.

Purpose: prove the reliability work with deterministic and real-repo scenarios.

Minimum deterministic scenarios:

1. `read -> edit -> read` ordering with changed content visible after edit.
2. Exact edit conflict: old text missing, with recovery suggestion.
3. Multiple match edit: requires replace-all or narrower anchor.
4. Partial read then edit outside read range.
5. Stale read after external file modification.
6. Shell mutation attempt blocked or escalated with clear reason.
7. Dangerous shell command denied and recovered.
8. Long command output stored as artifact and summarized.
9. Background dev server started, output read, then cancelled.
10. Permission denied once, then safe alternative succeeds.
11. Large diff attached as structured context and visible in trace.
12. Context compaction preserves changed files and validation state.

Real-project dogfood scenarios:

1. Small bug fix in this repo.
2. Multi-file Rust refactor with tests.
3. Desktop UI change with build and smoke validation.
4. Test failure repair from failing command output.
5. Permission-sensitive file change where user denies first path.
6. Long run with compaction or large output.

Validation:

```bash
cargo fmt --check
cargo test -q tool_contract
cargo test -q command_classifier
cargo test -q file_tool -- --test-threads=1
cargo test -q permissions
cargo test -q human_review
cargo test -q context_compressor
cargo test -q
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Acceptance:

- No release candidate can call itself Claude-like for programming quality
  until this gauntlet is green or each miss has a documented product decision.

## Execution Order

1. Track A: tool contract reliability audit and test gate.
2. Track B: structured file edit preview, conflict diagnostics, and rollback
   metadata.
3. Track C: bash mutation detection, subcommand facts, and terminal-task
   recovery.
4. Track D: permission evidence normalization and denial recovery.
5. Track F: desktop timeline/trace rendering of the new runtime facts.
6. Track E: compaction continuity for long coding sessions.
7. Track G: deterministic and dogfood gauntlet.

The immediate next implementation should start with Track A. It is the cheapest
way to find silent default semantics before deeper file and shell work builds on
top of them.

## Progress Log

### 2026-05-23

- Created this focused plan after comparing the current Priority Agent runtime
  with the local Claude source areas that matter for programming reliability:
  tool contract, file edit, bash, permission, query loop, and compaction.
- Current conclusion: the repo has the correct architecture, but needs a
  strict tool/file reliability audit and release gate before more UI polish.
- Audited the first version of this plan against the local Claude source and
  upgraded it from a capability checklist to a semantic replication plan:
  implementation batches now must extract guard order, state transitions,
  concurrency/abort boundaries, permission precedence, model/UI result splits,
  and error branches before coding.
- Started Track A with a side-effect-free tool reliability audit:
  - added `ToolReliabilityProfile`, representative tool samples, issue
    severity, and registry-level audit generation;
  - added a release tool contract test gate so release-visible tools cannot
    keep `Other` operation kind or unsafe concurrency/path/permission defaults;
  - wired `/audit tools` to show the registry reliability audit from the TUI;
  - fixed first audit findings by giving `git`, MCP tools, and `worktree`
    explicit operation kind, read-only/concurrency, matcher, summary, and UI
    render semantics.
- Started Track B with unified structured edit previews:
  - `file_write`, `file_edit`, and `file_patch` now expose `edit_preview`
    metadata with path identity, hashes, changed range, additions/deletions,
    diff preview, text format, validation stage, checkpoint id, file change id,
    and rollback handle;
  - targeted tests now pin the metadata shape so desktop timeline and trace
    views can render concrete file-edit evidence instead of generic success
    messages.

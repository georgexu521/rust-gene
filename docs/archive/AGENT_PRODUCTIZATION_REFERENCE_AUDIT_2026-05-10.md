# Agent Productization Reference Audit

Date: 2026-05-10

This document is the Batch 1 artifact for
`docs/NEXT_AGENT_PRODUCTIZATION_PLAN_2026-05-10.md`.

Purpose:

- Ground the next implementation batches in Claude Code and opencode source.
- Record what should be borrowed as product semantics.
- Record what should not be copied directly.
- Map the current Priority Agent runtime so the next refactors have clear
  boundaries.

## Current Snapshot

Local repo:

```text
cwd=/Users/georgexu/Desktop/rust-agent
branch=claude
head=0826458 Tighten focused repair guidance
src_files=282
docs_files=1328
scripts_files=20
```

Current uncommitted scope at the time of this audit:

```text
docs/NEXT_AGENT_PRODUCTIZATION_PLAN_2026-05-10.md
docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md
```

Current high-risk file sizes:

```text
6918 src/engine/conversation_loop/mod.rs
2369 src/engine/conversation_loop/patch_recovery.rs
1648 scripts/run_live_eval.sh
1291 src/tools/mod.rs
 881 src/engine/conversation_loop/step_executor.rs
 574 src/engine/conversation_loop/companion_context.rs
 538 src/tui/slash_handler/config.rs
```

The main execution loop has been reduced from the earlier 8600+ line state, but
it is still the central risk. The new plan should keep extracting product
services from the loop instead of adding more checkpoint branches inside it.

## Reference Note 1: Session Processor

### Claude Code References

- `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`
- `/Users/georgexu/Desktop/claude/src/Tool.ts`

Claude Code keeps a large `QueryEngine`, but it does not make the model-visible
workflow the only source of truth. The important product semantics are:

- `ToolUseContext` carries options, abort control, app state, file cache,
  tool UI hooks, notifications, MCP data, and memory/session utilities.
- `ToolPermissionContext` is separate from tool execution. Permission mode,
  allow/deny/ask rules, additional working directories, prompt-avoidance, and
  plan-mode state live in a permission context instead of prompt prose.
- File history is wired into the query engine, so file changes can be tracked
  without asking the model to describe rollback state.

### opencode References

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/prompt.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts`

opencode makes the split more explicit:

- `SessionPrompt` resolves prompt parts, agent, model, tools, permissions,
  subtasks, and shell entry points.
- `SessionProcessor` owns the lifecycle of streamed LLM events and tool calls.
- Tool calls have durable part state: pending/running/completed/error.
- Permission rejection and question rejection are first-class flow states, not
  generic failures.
- A snapshot is captured before the LLM stream starts so later tool execution
  has a before/after state boundary.

### Borrowed Semantics

Priority Agent should borrow these semantics:

- A session processor owns one turn lifecycle.
- Tool call state is explicit.
- Permission rejection is normal product flow.
- Snapshot/evidence capture happens before tool mutation.
- Tool execution remains a separate controller.

### Do Not Copy Directly

- Do not translate opencode's Effect service stack literally into Rust.
- Do not split into many tiny modules before the runtime seams are stable.
- Do not move all of `ConversationLoop` into a new `SessionProcessor` object
  unchanged.

### Priority Agent Landing

Current landing points:

- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/tool_orchestrator.rs`
- `src/engine/conversation_loop/tool_execution.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/turn_recording.rs`

Next landing points:

- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/evidence_ledger.rs`

First useful extraction:

- Move tool-call lifecycle bookkeeping out of `run_inner`:
  `exposed_tool_names`, failed-tool fingerprints, repeated failed tool names,
  per-round tool result collection, and successful validation command tracking.

## Reference Note 2: Tool Result Schema

### Claude Code References

- `/Users/georgexu/Desktop/claude/src/Tool.ts`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashToolResultMessage.tsx`

Claude Code's tool interface distinguishes:

- input schema;
- output schema;
- user-facing name;
- tool-use summary;
- activity description;
- validation;
- permission check;
- result rendering.

The key behavior is that a tool result is not just text. It is a product object
used by the UI, final response, permission layer, and session state.

### opencode References

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/registry.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/prompt.ts`

opencode wraps tool execution through a registry and session processor:

- tool definitions are filtered by model/provider/agent permission;
- tool execution updates durable tool part metadata;
- completed tool calls have title, metadata, output, attachments, and time;
- failed tool calls have explicit error state.

### Current Priority Agent State

Current `ToolResult` fields:

```text
success
content
error
error_code
data
duration_ms
tool_name
```

Current strengths:

- `tool_metadata.rs` already attaches `tool_summary`.
- Bash results include `audit`, `command_classification`, and execution data.
- Tool failures receive recovery metadata.
- Large tool results are truncated with UTF-8-safe prefix/suffix snippets.

Current gaps:

- Provider-safe serialization is not yet a clearly named boundary.
- Tool results mix user-visible text and machine evidence in one object.
- Tool-call lifecycle state is mostly in `run_inner`, not a durable controller.
- There is no single normalized result object comparable to opencode's tool
  part state.

### Borrowed Semantics

Priority Agent should introduce a normalized internal result shape:

```text
ToolExecutionRecord
  call_id
  tool_name
  input
  status=pending|running|completed|failed|blocked
  user_output
  machine_metadata
  evidence_kind
  permission_decision
  started_at
  ended_at
  provider_payload
```

The provider payload should be generated from this record, not hand-built in
multiple places.

### Do Not Copy Directly

- Do not tie result formatting to a React/terminal component model.
- Do not make every tool return a large custom object to the model.
- Do not expose internal metadata as model-visible text unless it is needed for
  the next reasoning step.

### Priority Agent Landing

Batch 2 should start here:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/tool_execution.rs`
- provider adapters under `src/services/api/`

Acceptance tests should cover:

- MiniMax/OpenAI-compatible tool result shape.
- error result with non-empty content.
- bash result with command/status/stdout/stderr metadata.
- file result with path and change evidence.

## Reference Note 3: Bash, Shell, and PTY

### Claude Code References

- `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`
- `/Users/georgexu/Desktop/claude/src/tasks/LocalShellTask/LocalShellTask.tsx`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/bashPermissions.ts`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/commandSemantics.ts`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/readOnlyValidation.ts`

Claude Code treats bash as a product-grade tool:

- it has strict input/output schema;
- it can classify read/search/list commands;
- it validates background needs for blocking commands;
- it uses permission matching over parsed subcommands;
- it has foreground/background shell tasks;
- it tracks sed-style edits into file history;
- it has UI summaries and progress labels.

### opencode References

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/shell/shell.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/pty/`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/arity.ts`

opencode's shell tool parses bash/PowerShell syntax with tree-sitter, scans
path arguments, asks permission for external directories and command patterns,
and records output through tool metadata. It also has a separate PTY layer for
interactive terminal semantics.

### Current Priority Agent State

Current strengths:

- `src/tools/bash_tool/mod.rs` executes commands with timeout and process-tree
  kill.
- The bash tool records audit metadata, command classification, backend, exit
  code, stdout/stderr length, and fallback reason.
- `src/tools/bash_tool/command_classifier.rs` already identifies validation
  families and command kinds.
- `tool_orchestrator.rs` exposes bash on code-change and bug-fix routes.
- `pseudo_tool_text.rs` catches false "bash unavailable" answers when bash is
  actually exposed.

Current gaps:

- Shell execution is still tool-shaped, not terminal-shaped.
- No PTY/task abstraction comparable to Claude `LocalShellTask` or opencode
  `pty/`.
- Permission currently sees dangerous command heuristics, but not a full
  parsed command/path scan.
- Route exposure diagnostics are not yet user-facing enough.
- The final answer can still rely on incomplete shell facts unless evidence is
  normalized.

### Borrowed Semantics

Priority Agent should borrow:

- command parse/classification before permission;
- read/list/search/mutation/install/run/destructive distinction;
- foreground command result and future background/PTY task as separate states;
- terminal availability diagnostics in `/status` or `/doctor`;
- command output as evidence, not just assistant text.

### Do Not Copy Directly

- Do not immediately implement a full tree-sitter shell parser if command
  classifier + path scanner covers the immediate failures.
- Do not add PTY before normal bash execution and result schema are stable.
- Do not expose a broad always-allow bash just because terminal must be
  available.

### Priority Agent Landing

Batch 3 should update:

- `src/tools/bash_tool/mod.rs`
- `src/tools/bash_tool/command_classifier.rs`
- `src/engine/conversation_loop/tool_orchestrator.rs`
- `src/tui/slash_handler/status` or diagnostics/doctor surfaces
- `src/diagnostics/mod.rs`

Immediate regression prompts:

```text
帮我看看我电脑默认的 python 有没有安装 pygame，帮我安装一下吧
我该怎么运行刚才创建的 Python 游戏？
cargo test 报错了，帮我修一下
```

## Reference Note 4: Permission and Risk UX

### Claude Code References

- `/Users/georgexu/Desktop/claude/src/components/permissions/`
- `/Users/georgexu/Desktop/claude/src/utils/permissions/`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/destructiveCommandWarning.ts`

Claude Code makes permission a user-visible product path:

- permission request title and explanation are UI components;
- shell-specific helper text explains why a command is risky;
- bypass/auto/default modes are explicit;
- permission context is passed separately from tool execution context.

### opencode References

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/agent/agent.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/config/permission.ts`

opencode keeps permission rules data-driven:

- `Request` includes session, permission, patterns, metadata, and tool call id;
- replies are `once`, `always`, or `reject`;
- pending approvals are tracked;
- rejection/correction/denial are separate error types;
- agent profiles merge defaults with user config.

### Current Priority Agent State

Current strengths:

- `src/permissions/` has permission mode and classifier plumbing.
- `src/engine/destructive_scope.rs` checks whether destructive commands match
  the latest user-approved target.
- `tool_orchestrator.rs` exposes tools through route and permission context.
- CLI approval panels exist for bash and file-write review flows according to
  current status docs.

Current gaps:

- Permission decisions are less productized than Claude/opencode.
- Rejection, correction, denial, and hidden-tool reasons are not consistently
  visible in `/status` or `/trace`.
- Agent/profile permissions exist, but they are not yet the primary mental
  model for all tool exposure.
- Install/network/destructive/publish risk categories should become explicit
  permission facts.

### Borrowed Semantics

Priority Agent should borrow:

- permission request as a durable object;
- once/always/reject replies;
- hidden tool reasons;
- correction feedback as a model input only when useful;
- agent/profile permission rules as data, not prompt instructions.

### Do Not Copy Directly

- Do not build a large UI before the permission data model is stable.
- Do not make permission prompts block background/subagent flows unless they
  can actually be shown to the user.

### Priority Agent Landing

Later Batch 4/5 work should touch:

- `src/permissions/mod.rs`
- `src/engine/destructive_scope.rs`
- `src/engine/conversation_loop/approval.rs`
- `src/engine/trace.rs`
- `src/tui/components/`
- `src/tui/slash_handler/permissions.rs`

## Reference Note 5: Evidence, Session Storage, and Recovery

### Claude Code References

- `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts`
- `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`

Claude's file history creates backups before edits, makes snapshots, can check
diffs, and can rewind/restore. The critical semantic is that recovery state is
captured by the runtime before mutation.

### opencode References

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/storage/`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/snapshot/index.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts`

opencode stores sessions, messages, parts, permissions, snapshots, and
migration state as product data. Tool results are session parts, not ephemeral
strings.

### Current Priority Agent State

Current strengths:

- `src/session_store/mod.rs` stores sessions, messages, learning events, and
  agent artifacts in SQLite with migrations.
- `src/engine/checkpoint.rs` already references Claude Code's `fileHistory.ts`
  pattern and stores file backups under `.priority-agent/checkpoints`.
- `src/tools/rewind_tool/mod.rs` and `src/tools/resume_tool/mod.rs` exist.
- `TurnTrace` captures prompt, routing, memory, context, tools, permissions,
  goal drift, assistant events, and MCP resources.

Current gaps:

- `rewind` and `resume` are still mostly surface-level responses.
- Checkpoint data, session store data, tool execution metadata, and closeout
  evidence are not yet unified as an evidence ledger.
- The final answer can reference verification without a single ledger object
  deciding what was proven.

### Borrowed Semantics

Priority Agent should introduce an `EvidenceLedger` that links:

- user request;
- route and exposed tools;
- tool calls and normalized results;
- file changes/checkpoints;
- validation commands;
- acceptance decisions;
- closeout status.

The ledger should not be a verbose prompt. It is runtime state used by trace,
closeout, eval, and recovery.

### Do Not Copy Directly

- Do not replace the existing SQLite store and checkpoint manager in one step.
- Do not expose all evidence to the model by default.
- Do not make the ledger a second workflow contract.

### Priority Agent Landing

Batch 4 should start with:

- `src/engine/conversation_loop/evidence_ledger.rs`
- `src/engine/conversation_loop/closeout_controller.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/auto_verify.rs`
- `src/session_store/mod.rs`
- `src/engine/checkpoint.rs`

## Current Priority Agent Architecture Map

### Main Runtime Spine

```text
src/engine/mod.rs
  -> ConversationLoopBuilder
  -> src/engine/conversation_loop/mod.rs
      -> prompt/context/retrieval setup
      -> intent route and workflow setup
      -> route-scoped tool exposure
      -> LLM request loop
      -> tool execution
      -> validation and repair
      -> closeout
      -> trace/session/memory persistence
```

### Already Extracted Conversation Loop Helpers

| File | Current role | Next decision |
|------|--------------|---------------|
| `action_checkpoint.rs` | focused repair/action checkpoint prompt and tool gating helpers | keep as repair-controller detail |
| `approval.rs` | approval request/channel wrapper | fold into permission controller later |
| `closeout_controller.rs` | final closeout application shell | grow into real closeout controller backed by evidence |
| `companion_context.rs` | surfaces nearby helper context after read/search | keep isolated, ensure evidence-only |
| `patch_recovery.rs` | LLM/deterministic patch synthesis and validators | keep isolated; reduce eval-specific rules over time |
| `patch_repair_rules.rs` | named deterministic repair rule registry | keep as explicit fallback boundary |
| `pseudo_tool_text.rs` | catches pseudo tool text and false bash/filesystem claims | move toward evidence/truth checker |
| `repair_controller.rs` | repair attempt accounting/context | grow into repair controller |
| `step_executor.rs` | legacy workflow step execution | evaluate for retirement or strict opt-in |
| `text_sanitizer.rs` | output text cleanup | keep utility |
| `tool_execution.rs` | read-only list, truncation, UTF-8-safe result helpers | split generic truncation vs execution state |
| `tool_metadata.rs` | execution summaries and recovery metadata | foundation for normalized tool result |
| `tool_orchestrator.rs` | route-scoped tool exposure and focused action tools | foundation for tool exposure policy |
| `turn_recording.rs` | learning/recovery/goal/MCP/web trace recording | keep as persistence adapter |
| `validation_runner.rs` | shell validation helper and auto-test predicates | split into validation controller later |

### Responsibilities Still Inside `conversation_loop/mod.rs`

The following responsibilities remain in the main file and should be extracted
in small, behavior-preserving batches:

1. API request loop and streaming/non-streaming fallback.
2. Per-turn tool-call lifecycle bookkeeping.
3. Tool result message formatting and provider payload handling.
4. Required validation command tracking.
5. Action checkpoint activation and patch-only mode decisions.
6. Runtime diet telemetry collection.
7. Pseudo-tool correction retry logic.
8. Destructive scope check integration.
9. Final loop termination and closeout trigger decisions.
10. Session/memory/trace persistence orchestration.

### Product Capability Table

| Capability | Current state | Reference target | Next move |
|------------|---------------|------------------|-----------|
| Prompt assembly | compact base prompt, AGENTS section loading, prompt budget tests | Claude/opencode composable prompt parts | keep short; do not add new always-on rules |
| Tool exposure | route-scoped tools enabled by default | opencode agent permission + registry filtering | add hidden-tool diagnostics |
| Tool result | `ToolResult` plus `tool_summary` metadata | durable tool part/result object | Batch 2 normalized result and provider-safe serialization |
| Bash | timeout, backend, audit, classifier, dangerous heuristic | Claude BashTool + opencode ShellTool/PTY | Batch 3 diagnostics, command kind, future PTY |
| Permission | permission modes, destructive scope, approval channel | explicit request/reply/ruleset UI | make hidden/denied reasons visible |
| Evidence | trace, closeout counts, auto verify, checkpoints | file history + session parts + snapshot | Batch 4 EvidenceLedger |
| Session | SQLite sessions/messages/learning/events | storage/session/message/part + restore | connect session store to evidence ledger |
| Recovery | focused repair, patch synthesis, validation repair | failed tool parts + targeted retry | reduce model-visible repair text |
| Eval | live eval parser, aggregate reports, deterministic matrix | product comparison on real tasks | add hallucination/terminal behavior signals |

## Batch 1 Decisions

1. Batch 2 should start with provider-safe tool result normalization.
   This is the strongest shared prerequisite because both Claude and opencode
   treat tool results as structured product state.

2. Batch 3 should improve bash as a terminal product surface, not just expose
   it more broadly. The first step is diagnostics and schema, not full PTY.

3. Batch 4 should introduce an `EvidenceLedger` as runtime state. It should
   not become another model-visible workflow contract.

4. `ConversationLoop` extraction should follow product seams:
   tool execution lifecycle first, then evidence/closeout, then repair.

5. Every future batch should include a short reference note. If the reference
   path changes, update this document instead of relying on old memory.

## Immediate Next Work

Recommended next implementation order:

1. Batch 2: normalize tool result and provider payloads.
2. Batch 3: terminal availability diagnostics and bash route exposure checks.
3. Batch 4: `EvidenceLedger` first version.
4. Batch 5: first behavior-preserving `ConversationLoop` extraction.

Validation for this audit-only batch:

```bash
cargo fmt --check
git diff --check
```

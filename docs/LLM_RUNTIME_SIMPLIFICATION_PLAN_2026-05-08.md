# LLM Runtime Simplification Plan

Date: 2026-05-08

## Goal

Make Priority Agent feel less like a model forced through a process manual and
more like a capable coding assistant with reliable runtime guardrails.

The target architecture is:

- Runtime owns safety, scope, evidence, validation, and rollback switches.
- The model owns understanding, judgment, code generation, and concise user
  communication.
- Prompt text supplies only stable behavioral boundaries and current task
  context.
- Heavy workflow machinery activates only when risk, failure, ambiguity, or
  explicit user intent justifies it.

This plan is written against the current checkout on branch `claude`. The
worktree already contains unrelated in-progress cleanup and validation changes;
implementation should preserve those changes and avoid reverting them.

## Document Decision

Continue using this file instead of creating a second plan.

Reason: the Claude/opencode review is not a separate project. It is a follow-up
input to the same runtime-simplification goal: keep the model free to solve the
task while moving safety, permissions, validation, and evidence into runtime
guardrails. A new document would split execution state across two plans and make
it harder to tell which phase is next.

Append new work as Phase 9 and later. Leave completed Phase 0-8 records in this
file as the audit trail.

## Current Findings

### 1. The base system prompt is still too instructional

`src/engine/mod.rs:80-204` injects a large set of behavioral rules, workflow
guidance, tool instructions, examples, and safety notes into the model context.
Some of these rules are valuable, but too many are process details:

- `src/engine/mod.rs:90-118` asks the model to internally handle task
  completeness, weighting, acceptance criteria, guided reasoning, and residual
  risks.
- `src/engine/mod.rs:120-176` repeats tool usage instructions that could be
  encoded in tool descriptions, tool exposure, and runtime rejection messages.
- `src/engine/mod.rs:178-184` contains important destructive-scope guidance,
  but this should be enforced by runtime checks rather than only prompt text.

Risk: the model spends attention on framework compliance instead of the user's
literal task, and prompt-only rules fail silently when the model drifts.

### 2. Workflow judgment can inject another process layer

`src/engine/workflow_contract.rs:778-824` converts workflow judgment into a
turn-context block containing task type, complexity, risk, questions,
assumptions, prioritized plan, weights, and acceptance criteria.

`src/engine/workflow_contract.rs:859-944` asks a model to generate a structured
JSON judgment with priority, weight factors, assumptions, plan, and acceptance
contracts.

This is appropriate for broad or risky work, but it is too heavy for common
tasks such as:

- delete this one file
- create a small standalone script
- explain a local error
- make a tiny local edit

Risk: the model receives a second "manager" prompt before doing the actual
task, and the generated structure can become the task instead of support for
the task.

### 3. Code-change policy is lightweight in name, but closeout is still on by default

`src/engine/code_change_workflow.rs:119-170` already distinguishes strict,
medium, low, and non-programming workflows. That is the right direction.

However, both medium- and low-risk programming workflows still set
`require_final_closeout: true` at `src/engine/code_change_workflow.rs:143` and
`src/engine/code_change_workflow.rs:154`.

`src/engine/code_change_workflow.rs:500-665` then formats closeout with
validation, acceptance, and residual-risk sections even when the user wanted a
simple result.

Risk: ordinary coding turns produce process-heavy output. Worse, status can
look authoritative even when the underlying evidence is thin.

### 4. Tool exposure is still broad for normal turns

The registry now has `ToolRegistryProfile::Core` and `Full`
(`src/tools/mod.rs:817-985`), which is useful. But Core still exposes a large
general-purpose surface: tasks, agent, web, memory, todo, cost/config/context,
git, notebook, REPL, PowerShell, MCP, LSP, symbol query, worktree, workbench,
refactor, project list, skills, and ask.

`ConversationLoop::get_tools` filters only by availability, allowed tools, and
permission exposure (`src/engine/conversation_loop/mod.rs:5465-5485`). It does
not yet use `IntentRoute.recommended_tools` to shrink the model-visible tool
set for a simple turn.

Risk: the model sees too many possible actions and can over-select tools that
are technically available but irrelevant to the user's current intent.

### 5. Intent routing misses natural Chinese creation requests

`src/engine/intent_router.rs:205-230` routes code changes when English code
verbs appear or Chinese terms such as `实现`, `新增`, `修改`, `优化`, `完善`,
`开发` appear.

The live issue used natural wording like "帮我做一个贪吃蛇游戏吧，用 python 做吧".
That contains "做" and "python", but not the current code-change Chinese
keywords. This can fall through to Direct while the model still calls write
tools.

Risk: runtime policy and validation are weaker exactly on common user phrasing.

### 6. Validation is improving, but evidence semantics need to be stricter

The recent patch added standalone Python file validation:

- changed `.py` files are detected at `src/engine/auto_verify.rs:169` and
  `src/engine/auto_verify.rs:843-865`
- `python3 -m py_compile` runs at `src/engine/auto_verify.rs:934-993`

That fixes one concrete false-positive closeout path. But the broader rule is
not yet explicit: a code-generation task should not be considered passed when
the verification set is empty, unless the runtime can prove verification was
not applicable and the final response says so.

Risk: future language/file types can repeat the same "empty evidence became
passed" pattern.

### 7. Destructive scope is still mostly model-guided

`src/engine/goal_drift.rs:48-156` detects destructive-looking shell commands
and broad intent drift. This catches some dangerous calls, but it does not yet
represent the exact user-approved destructive target.

The screenshot problem was not only "rm was dangerous"; deleting `abc.txt` was
allowed, but the model then suggested deleting the parent folder. That requires
a scope contract: user approved `abc.txt`, not its parent or siblings.

Risk: prompt text can reduce this behavior, but only runtime scope comparison
can reliably prevent it.

### 8. There are two workflow stacks

The main code-change path uses `IntentRouter`, `TaskContextBundle`,
`CodeChangeWorkflowRunner`, validation, reflection, and closeout.

There is also a legacy/general workflow gate in
`src/engine/workflow/gate.rs:1-280`, where no fast-lane or heuristic match
defaults to Workflow when enabled. `src/engine/workflow/policy.rs` currently
defaults `PRIORITY_AGENT_WORKFLOW_ENABLED` to false, so this path is mostly
opt-in. Still, it adds conceptual weight and makes the runtime harder to reason
about.

Risk: old workflow concepts keep leaking into new behavior and docs, making it
hard to tell which layer actually controls the turn.

## Design Principles

1. Keep hard safety outside the model.
   Runtime must own destructive scope, permissions, path boundaries, and command
   risk.

2. Keep evidence outside the model.
   Runtime must decide whether validation actually ran and what it proved. The
   model may summarize evidence, but cannot invent it.

3. Keep prompts short and task-shaped.
   Prompt text should say what matters now, not restate the full operating
   manual every turn.

4. Make heavy workflow opt-in by risk.
   Strict plan/acceptance/review loops should activate on high risk, failed
   verification, repeated no-progress, explicit planning, or required commands.

5. Prefer route-scoped tools.
   The model should see the smallest useful tool set for the current intent,
   with a debug/full profile available when needed.

6. Keep final answers natural.
   Structured closeout is useful internally and in `/trace`; user-facing output
   should be concise unless there is failure, uncertainty, or explicit request
   for evidence.

## Implementation Plan

### Phase 0 - Baseline and regression cases

Purpose: make the current problem measurable before changing behavior.

Tasks:

1. Add deterministic route/validation tests for common natural requests:
   - `帮我做一个贪吃蛇游戏吧，用 python 做吧`
   - `帮我把这个文件删了吧`
   - `创建一个简单 html 页面`
   - `修复 cargo test 失败`
   - `帮我看看这段报错`
2. Add an execution-level regression fixture for:
   - single-file delete should not suggest deleting parent directory
   - standalone Python file creation must run `py_compile`
   - simple code creation should produce concise final text, not verbose
     closeout by default
3. Capture current per-turn prompt/tool statistics:
   - base system prompt chars/tokens
   - workflow context chars/tokens
   - exposed tool count by route
   - final answer closeout visibility

Likely files:

- `src/engine/intent_router.rs`
- `src/engine/auto_verify.rs`
- `src/engine/code_change_workflow.rs`
- `src/engine/conversation_loop/mod.rs`
- `scripts/run_live_eval.sh` or a smaller deterministic smoke script

Validation:

- `cargo test -q intent_router`
- `cargo test -q auto_verify`
- `cargo test -q code_change_workflow`

### Phase 1 - Prompt diet

Status: batch 3 completed on 2026-05-08.

Purpose: reduce always-on model instructions.

Tasks:

1. Split `src/engine/mod.rs` prompt into:
   - core assistant identity and basic behavior
   - safety boundary summary
   - optional coding guidance
   - optional tool usage guidance
   - Implemented as a compact always-on prompt with core conduct,
     model-led workflow, verification/reporting, scoped destructive action,
     and response format sections.
2. Remove or shorten always-on examples for `file_edit`, `file_write`, and
   tool usage. Move detailed repair guidance to targeted runtime messages only
   after relevant failures.
   - Implemented. The `file_edit` exact-match and line-range examples are no
     longer in the base prompt; repair-specific guidance remains in focused
     repair prompts and failure feedback.
3. Keep only stable always-on rules:
   - read before changing when needed
   - verify before claiming completion
   - do not hide failures
   - destructive actions require exact scope and approval
   - Implemented.
4. Add a prompt-size test with a budget threshold.
   - Implemented with a 700-token regression budget and a test that blocks
     always-on repair details from returning.

Likely files:

- `src/engine/mod.rs`
- `src/engine/conversation_loop/action_checkpoint.rs`
- `src/engine/conversation_loop/mod.rs`

Acceptance:

- Base system prompt is materially shorter: extracted prompt block dropped from
  7,687 chars to 2,732 chars.
- File-edit repair details appear only after edit failure or focused repair
  mode, not in every turn.
- Existing tool-use tests still pass.

Validation:

- `cargo test -q prompt`
- `cargo check -q`

Actual validation for Batch 3:

```bash
cargo fmt --check
cargo test -q prompt
cargo check -q
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
cargo clippy --all-features -q -- -D warnings
```

Result: prompt tests passed with `31 passed; 0 failed`; full default tests
passed with `1048 passed; 0 failed`.

### Phase 2 - Route-scoped tool exposure

Status: batch 2 completed on 2026-05-08.

Purpose: show the model fewer irrelevant tools.

Tasks:

1. Introduce a `ToolExposureProfile` selected from `IntentRoute` and
   `ResourcePolicy`.
   - Implemented as `ConversationLoop::route_scoped_tools`, keyed primarily by
     `IntentRoute`.
2. For simple direct turns, expose no tools or only read-only basics.
   - Implemented. Direct turns with no recommended tools expose no tools.
     Scoped file mutation turns expose only `file_read`, `glob`, `bash`, and
     `ask_user`.
3. For file creation/editing turns, expose:
   - `file_read`, `file_write`, `file_edit`
   - `bash` only when validation is likely needed
   - search/project tools only when project context is needed
   - Implemented conservatively for code-change routes: write/edit/read/search
     plus validation and local coding aids are visible; web, memory, MCP,
     delegation, plugin, desktop, telemetry, team, voice, and remote tools are
     hidden by default.
4. For debugging turns, expose search/read/bash first, then edit tools after
   evidence or user intent is clear.
   - Implemented with edit tools visible for `BugFix` routes because the router
     currently treats debugging prompts as fix intent. A later refinement can
     split inspection-only debugging from explicit repair.
5. Keep `PRIORITY_AGENT_TOOL_PROFILE=full` as the escape hatch, but make it
   additive rather than the default model-visible surface.
   - Implemented. `PRIORITY_AGENT_TOOL_PROFILE=full`,
     `PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE=1`, or
     `PRIORITY_AGENT_ROUTE_SCOPED_TOOLS=0` bypasses route-scoped filtering.

Likely files:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/resource_policy.rs`
- `src/engine/intent_router.rs`

Acceptance:

- A simple delete request does not expose web, memory, MCP, agent, swarm,
  worktree, plugin, or unrelated tools.
- A standalone Python script request exposes write + validation tools without
  unrelated platform tools.
- Debug/full mode can still expose the broader set.

Validation:

- `cargo test -q route_scoped_tools`
- `cargo test -q intent_router`
- `cargo fmt --check`
- `cargo clippy -q -- -D warnings`

Actual validation for Batch 2:

```bash
cargo fmt --check
cargo test -q route_scoped_tools
cargo test -q intent_router
cargo check -q
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
cargo clippy --all-features -q -- -D warnings
```

Result: `cargo test -q` passed with `1046 passed; 0 failed`.

### Phase 3 - Intent routing fixes for natural creation requests

Purpose: make runtime policy match how gex actually asks for work.

Tasks:

1. Treat Chinese `做`, `写`, `生成`, `创建`, `弄一个`, `做一个` plus code
   nouns/languages as code creation.
2. Treat language names/extensions as code hints when paired with a creation
   verb:
   - python, py, html, js, ts, rust, shell
3. Distinguish "explain how to make X" from "make X".
4. Add route tests for Chinese natural wording.

Likely files:

- `src/engine/intent_router.rs`

Acceptance:

- `帮我做一个贪吃蛇游戏吧，用 python 做吧` routes to `CodeChange`.
- `这个 python 报错什么意思` routes to `Debugging` or Direct explanation
  depending on wording, without forcing file changes.
- `计划一下怎么做贪吃蛇` routes to Planning, not CodeChange.

Validation:

- `cargo test -q intent_router`

### Phase 4 - Runtime evidence semantics

Purpose: prevent "empty verification means passed".

Tasks:

1. Add explicit evidence categories:
   - `verified`
   - `not_applicable`
   - `not_available`
   - `not_run`
   - `failed`
2. Update closeout status calculation so code-generation cannot pass solely
   because no verifier returned a result.
3. Extend language/file validation:
   - Python: keep `py_compile` for standalone `.py`
   - HTML/CSS/JS single files: at least existence + lightweight syntax/parse
     checks when feasible
   - shell: `bash -n`
   - JSON/YAML/TOML: parser validation
4. Make missing verification produce concise user-facing wording:
   - "Created X. I did not run it because no runtime/test command was
     available."
   not a full verbose closeout.

Likely files:

- `src/engine/auto_verify.rs`
- `src/engine/code_change_workflow.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/tools/bash_tool/command_classifier.rs`

Acceptance:

- Empty verification cannot produce `Status: passed` for a code-generation
  turn unless marked `not_applicable` with a reason.
- Single-file Python creation records `py_compile` evidence.
- Validation evidence is traceable in `/trace`.

Validation:

- `cargo test -q auto_verify`
- `cargo test -q code_change_workflow`

### Phase 5 - Closeout visibility simplification

Purpose: keep final answers natural while preserving traceability.

Tasks:

1. Add `CloseoutVisibility`:
   - `Hidden`: no user-facing closeout for simple successful turns
   - `Concise`: one sentence or compact evidence line
   - `Full`: current structured closeout, used for failures, high-risk work,
     live evals, or explicit `/trace`/debug settings
2. Keep structured closeout in `TurnTrace` and learning events.
3. Make final response formatter route-aware:
   - simple success: "Done. Created ..."
   - success with validation: "Done. Verified with ..."
   - not verified: concise caveat
   - failed: concise failure with next action
4. Add tests that ordinary single-file tasks do not print full `Closeout:`.

Likely files:

- `src/engine/code_change_workflow.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/tui/slash_handler/learning.rs`
- `src/engine/trace.rs`

Acceptance:

- The screenshot-style Python game task no longer ends with a large closeout
  block in normal mode.
- `/trace` and `/quick` still show closeout/evidence state.
- Live eval mode can request full closeout for scoring.

Validation:

- `cargo test -q closeout`
- `cargo test -q trace`

### Phase 6 - Destructive scope contract

Status: batch 4 completed on 2026-05-08.

Purpose: make delete/remove/reset scope runtime-enforced.

Tasks:

1. Extract destructive targets from the latest user request:
   - files/paths/refs explicitly named
   - whether parent directory or recursive cleanup was requested
   - Implemented in `src/engine/destructive_scope.rs`. The extractor handles
     explicit path/file tokens such as `abc.txt`, `./file`, absolute paths, and
     `~/...`, plus singular references like `这个文件`.
2. Compare destructive tool calls against approved targets.
   - Implemented for shell-backed destructive operations (`rm`, `trash`, `mv`,
     `git reset`, `git clean`) and worktree removal. Relative shell targets
     respect the tool call's `working_dir`.
3. If a tool call targets a parent/sibling/broader path, block or require
   explicit user approval.
   - Implemented as a hard pre-execution block when the target is broader than
     the request scope. Exact matches still continue to the normal permission
     approval path.
4. Add a post-action response guard:
   - after completing a destructive task, do not ask/suggest broader
     destructive follow-up unless user requested cleanup.
   - Implemented by injecting a post-action system guard after successful
     scoped destructive operations.
5. Trace scope decisions.
   - Implemented as `TraceEvent::DestructiveScopeChecked`.

Likely files:

- `src/engine/goal_drift.rs`
- `src/engine/human_review.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/permissions/mod.rs`

Acceptance:

- `删除 abc.txt` permits deleting `abc.txt` after approval, but not `~/Desktop/gex`.
- The assistant does not suggest deleting parent folders after a single-file
  delete.
- Broader cleanup still works when user explicitly asks for it.

Validation:

- `cargo test -q goal_drift`
- `cargo test -q permissions`

Actual validation for Batch 4:

```bash
cargo fmt --check
cargo test -q destructive_scope
cargo test -q goal_drift
cargo test -q permissions
cargo check -q
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
cargo clippy --all-features -q -- -D warnings
```

Result: destructive scope tests passed with `11 passed; 0 failed`; goal drift
tests passed with `4 passed; 0 failed`; permissions tests passed with
`46 passed; 0 failed`; full default tests passed with `1058 passed; 0 failed`.

### Phase 7 - Retire or isolate the legacy workflow stack

Status: completed on 2026-05-08.

Purpose: reduce conceptual overlap.

Tasks:

1. Decide whether `src/engine/workflow/*` is still product-path or eval-only.
   Decided: legacy/eval-only.
2. If eval-only, feature-gate or rename it so normal interactive CLI does not
   imply two workflow systems.
   Done: default policy and `Gate::new()` keep legacy workflow disabled; new
   explicit switch is `PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=1`, with
   `PRIORITY_AGENT_WORKFLOW_ENABLED=1` preserved for old scripts.
3. Remove the default-to-workflow behavior in `src/engine/workflow/gate.rs`
   when no fast-lane or heuristic match exists.
   Done: unmatched requests stay Direct even when legacy workflow is enabled.
4. Keep benchmark/live-eval compatibility through explicit env flags.
   Done: `scripts/run_live_eval.sh` now exports the legacy switch while keeping
   the old compatibility variable.

Likely files:

- `src/engine/workflow/gate.rs`
- `src/engine/workflow/policy.rs`
- `src/engine/conversation_loop/mod.rs`
- docs under `docs/workflow/`

Acceptance:

- Normal interactive CLI has one obvious execution path.
- Legacy workflow tests still pass under explicit feature/env mode or are
  documented as historical.

Validation:

- `cargo test -q workflow::`
- `cargo check -q`

Actual validation for Phase 7:

```bash
cargo fmt --check
cargo test -q workflow::
cargo test -q runtime_diet_report_is_recorded_for_real_loop_turn
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
cargo check -q
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
cargo clippy --all-features -q -- -D warnings
```

Result: `cargo test -q workflow::` passed with `88 passed; 0 failed`;
`cargo test -q` passed with `1063 passed; 0 failed`; all listed
commands passed.

### Phase 8 - Product-facing measurement

Status: completed on 2026-05-08.

Purpose: prevent future prompt/framework creep.

Tasks:

1. Add a small "runtime diet" report:
   - prompt token estimate
   - exposed tool count
   - workflow context injected yes/no
   - closeout visibility
   - validation evidence state
   Done via `TraceEvent::RuntimeDietReport`.
2. Surface this in `/trace` or `/quick` without making normal final answers
   verbose.
   Done via `/trace last` summary and `/quick` runtime panel.
3. Add a regression threshold for prompt/tool bloat.
   Done with trace-level light/heavy thresholds and regression tests.

Likely files:

- `src/engine/trace.rs`
- `src/tui/slash_handler/learning.rs`
- `src/tools/context_vis_tool/mod.rs`

Acceptance:

- We can see whether a turn was lightweight or heavy.
- A future change that adds large always-on context has a test or report signal.

Validation:

- `cargo test -q trace`
- `cargo test -q context`

Actual validation for Phase 8:

```bash
cargo fmt --check
cargo test -q trace
cargo test -q quick
cargo test -q context
cargo check -q
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
cargo clippy --all-features -q -- -D warnings
```

Result: `cargo test -q` passed with `1060 passed; 0 failed`; all listed
commands passed.

## Reference Project Follow-Up Findings

This section was added after inspecting the local reference repositories:

- `/Users/georgexu/Desktop/claude`
- `/Users/georgexu/Desktop/opencode-dev`

### Claude Code pattern

Claude Code does not avoid rules; it keeps rules at the right boundary.

Observed patterns:

1. Static and dynamic prompt layers are separated.
   `src/constants/prompts.ts` uses a dynamic boundary so stable prompt text can
   stay cacheable while volatile session details are resolved after the boundary.
   Dynamic sections go through a section registry, and uncached sections must be
   named and justified.
2. The always-on behavioral prompt is opinionated but concise.
   Important guidance is about boundaries: do not gold-plate, do not add
   speculative abstractions, read before editing, diagnose failures before
   changing approach, and confirm hard-to-reverse actions.
3. Tool-specific constraints live in tool descriptions and tool implementation.
   File edit rules such as read-before-edit, exact replacement, unique old
   string, and prefer-edit-over-write are attached to the edit tool instead of
   being repeated as global process text.
4. Different roles see different tool surfaces.
   Subagents, coordinator mode, and async agents have explicit allowlists. The
   model is not asked to ignore irrelevant tools; irrelevant tools are simply not
   visible.

Borrowable idea: make dynamic prompt injection explicit and auditable, and make
tool/role boundaries enforce constraints instead of turning the system prompt
into a process manual.

### opencode pattern

opencode is even more direct: simple base prompts, compact environment blocks,
agent-specific permission sets, and tool descriptions that carry local usage
rules.

Observed patterns:

1. Base prompt is selected by model/provider family.
   `packages/opencode/src/session/system.ts` chooses a model-appropriate prompt
   and appends a compact environment block.
2. Instruction files are included conservatively.
   `packages/opencode/src/session/instruction.ts` loads global instructions,
   then the first project-level `AGENTS.md`/`CLAUDE.md` match, plus explicit
   config instructions. It does not blindly stack every ancestor instruction
   file into the main system prompt.
3. `AGENTS.md` guidance is intentionally compact.
   The `/init` template says every line should answer whether an agent would
   likely miss that fact without help. Generic advice, long tutorials, exhaustive
   file trees, and unverifiable claims should be omitted.
4. Permissions and agent modes carry most hard constraints.
   Build, plan, explore, compaction, title, and summary agents all have distinct
   permission rules. Plan mode denies edits except the plan file; explore mode is
   mostly read/search only.

Borrowable idea: project instructions should contain only high-signal repo
facts, while permission/agent rules carry the hard behavior constraints.

### Priority Agent delta

The base prompt is now short enough, but the project instruction layer can still
undo that work.

Current risk:

1. `AGENTS.md` is about 46 KB and begins with product philosophy, old workflow
   diagrams, Socratic/weighting descriptions, historical roadmap notes, and
   stale tool counts.
2. `src/instructions/mod.rs` injects up to 4,000 chars per instruction layer and
   up to 16,000 chars total. Because the root `AGENTS.md` starts with framework
   philosophy, normal turns can still receive old "weighted priority +
   Socratic" guidance even after the base prompt was dieted.
3. `CodeChangeWorkflowRunner` still creates an internal closeout for normal
   programming turns. User-facing output is now more concise, but the runtime can
   still nudge the model toward framework completion semantics.
4. Route-scoped tools help, but code-change routes still expose a broad local
   coding surface. This is safer than the old global registry, but still heavier
   than Claude/opencode-style role-specific minimal surfaces.

The next work should focus on project-instruction diet first, then instruction
loading, then finer tool/agent/closeout gating.

## Follow-Up Phases

### Phase 9 - AGENTS.md runtime diet

Status: completed on 2026-05-08.

Purpose: prevent project instructions from reintroducing old workflow doctrine
into every model turn.

Tasks:

1. Rewrite root `AGENTS.md` as a compact runtime guide.
   Keep only facts an agent is likely to miss without help:
   - current product name and CLI naming
   - canonical status docs
   - exact build/run/test commands
   - high-value validation gates
   - core execution entrypoints
   - repo-specific conventions and current cleanup plan
2. Move historical/product-philosophy content out of `AGENTS.md`.
   Candidate destinations:
   - `docs/PROJECT_HISTORY.md` for historical timeline
   - `docs/ARCHITECTURE_NOTES.md` for weight/Socratic product theory
   - existing status/roadmap docs for validated state
3. Remove or relocate stale sections:
   - old tool-count claims
   - old Phase 1-4 roadmap records
   - detailed tree listings that are easy to rediscover
   - "complete Socratic workflow" instructions that should not be imposed every
     turn
4. Add a size and content regression test.
   Target:
   - root `AGENTS.md` should stay below 4,000 chars unless explicitly exempted
   - prompt-visible project instructions should not include old framework
     phrases such as `完整的 Socratic 执行流程` or `高密度思考 = 高密度提问-解答`
5. Update `docs/PROJECT_STATUS.md` or the appropriate status doc only if this
   changes documented startup/validation guidance.

Likely files:

- `AGENTS.md`
- `docs/PROJECT_HISTORY.md` or `docs/ARCHITECTURE_NOTES.md`
- `src/instructions/mod.rs`
- tests near `src/instructions/mod.rs`

Acceptance:

- Prompt-visible project instructions become short and practical.
- Product philosophy and old roadmap content remain available in docs, but are
  not injected into every LLM turn.
- A future oversized `AGENTS.md` fails a test or appears as a runtime diet
  warning.

Implementation notes:

- Replaced root `AGENTS.md` with a compact runtime guide focused on current
  project facts, entrypoints, validation commands, and the active over-control
  cleanup line.
- Archived the previous long project guide at
  `docs/archive/AGENTS_PROJECT_GUIDE_PRE_RUNTIME_DIET_2026-05-08.md`.
- Added instruction-loader regression tests that keep root `AGENTS.md` under
  the prompt-visible per-layer budget and prevent archived workflow doctrine
  phrases from leaking back into prompt-visible project instructions.
- Current root `AGENTS.md` size after the rewrite and Phase 10 section marker:
  2941 bytes.

Validation:

- `cargo test -q instructions`
- `cargo test -q prompt`
- `cargo test -q quick`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q instructions` - passed, 7 tests.
- `cargo test -q prompt` - passed, 32 tests.
- `cargo test -q quick` - passed, 1 test.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.

### Phase 10 - Section-based instruction loading

Status: completed on 2026-05-08.

Purpose: make instruction injection explicit instead of prefix-truncating large
files.

Tasks:

1. Add a preferred prompt-visible section format.
   Proposed section:
   - `## Agent Runtime Guidance`
   If present, inject only this section from `AGENTS.md`.
2. Preserve current fallback behavior for repositories without that section.
   Fallback may still clip by char limit, but the trace should label it as a
   fallback/truncated layer.
3. Add layer diagnostics:
   - source path
   - injected section name or fallback mode
   - original chars
   - injected chars
   - truncated yes/no
4. Surface diagnostics in existing prompt/runtime diet reports without making
   normal final answers verbose.
5. Add tests:
   - section extraction wins over prefix clipping
   - missing section falls back to current behavior
   - oversized fallback is flagged
   - workspace boundary text remains injected

Likely files:

- `src/instructions/mod.rs`
- `src/engine/prompt_context.rs`
- `src/engine/trace.rs`
- `src/tui/slash_handler/learning.rs`

Acceptance:

- A long `AGENTS.md` can contain archived context without polluting the model
  turn when `## Agent Runtime Guidance` exists.
- `/quick` or `/trace` can show when instruction loading used a fallback or
  truncation path.
- Existing projects without the section continue to work.

Implementation notes:

- Added `## Agent Runtime Guidance` to root `AGENTS.md`; normal instruction
  loading now selects only that section when present.
- Preserved existing full-file fallback behavior for projects without the
  section.
- Added `PRIORITY_AGENT_AGENTS_MD_FULL=1` as a diagnostic rollback path for
  full-file loading.
- Added `InstructionLayerSelection` diagnostics with runtime-guidance,
  fallback, env override, selected char count, and truncation state.
- Surfaced the selection mode in prompt context layer reports, e.g.
  `AGENTS.md [project:runtime-guidance]`.
- Added tests for section preference, fallback behavior, root-project section
  use, archived-content exclusion, and prompt-context layer labels.

Validation:

- `cargo test -q instructions`
- `cargo test -q prompt_context`
- `cargo test -q quick`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q instructions` - passed, 10 tests.
- `cargo test -q prompt_context` - passed, 4 tests.
- `cargo test -q prompt` - passed, 33 tests.
- `cargo test -q quick` - passed, 1 test.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.

### Phase 11 - Move local rules into tool contracts

Status: completed on 2026-05-08.

Purpose: keep the system prompt short by putting local usage constraints in the
tools that enforce them.

Tasks:

1. Audit core tool descriptions against Claude/opencode:
   - `file_read`
   - `file_write`
   - `file_edit`
   - `bash`
   - `agent`
   - `skill_view`
2. Strengthen tool-local guidance only where it prevents real errors:
   - read output line prefixes must not be copied into edit strings
   - edit requires prior read and exact match
   - write is for new files, edit is for existing files
   - bash is for shell-only operations and validation, not user-facing
     communication
3. Prefer runtime errors and correction messages over global prompt text.
   Example: if `file_edit` fails because the old string is not unique, the tool
   result should tell the model exactly how to retry.
4. Track tool schema/description bloat.
   If a description grows too large, move rare guidance into failure-specific
   messages instead.

Likely files:

- `src/tools/file_tool/mod.rs`
- `src/tools/bash_tool/mod.rs`
- `src/tools/agent_tool/mod.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/trace.rs`

Acceptance:

- Base prompt does not need detailed edit/write examples.
- Core tools return actionable local errors when the model misuses them.
- Tool schema token count remains inside runtime diet thresholds.

Implementation notes:

- `file_edit` now detects `file_read` display line prefixes such as `12 |` in
  `old_string`, `insert_after`, or `insert_before` anchors and returns a
  correction that tells the model to remove the prefix or use
  `line_start`/`line_end`.
- `file_write` contract now says it is best for new files or intentional
  full-file replacement. When it overwrites an existing file, the result data
  records `existed_before=true` and guidance to use `file_edit` for targeted
  existing-file changes.
- `bash` contract now scopes bash to shell-only operations, validation, and git
  commands, and clarifies that bash output is not user-facing communication.
- `agent` contract now discourages blocking delegation and steers use toward
  independent, parallel, bounded tasks with narrow tool allowlists.
- `skill_view` contract now fences skill text as task-relevant background
  guidance, not user instruction.
- Added a compactness regression test for the core tool descriptions so rare
  guidance is pushed into failure-specific messages instead of growing schemas.

Validation:

- `cargo test -q file_tool`
- `cargo test -q bash_tool`
- `cargo test -q route_scoped_tools`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q file_tool` - passed, 26 tests.
- `cargo test -q bash_tool` - passed, 16 tests.
- `cargo test -q route_scoped_tools` - passed, 4 tests.
- `cargo test -q agent_tool_contract` - passed, 1 test.
- `cargo test -q skill_view_contract` - passed, 1 test.
- `cargo test -q core_tool_contract_descriptions_stay_compact` - passed, 1
  test.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.
- `git diff --check` - passed.
- `cargo test -q` - passed, 1081 tests.

### Phase 12 - Role-specific tools and permissions

Status: completed on 2026-05-08.

Purpose: make tool access match task role instead of relying on the model to
self-select from a broad surface.

Tasks:

1. Define built-in role profiles:
   - primary/default
   - explorer
   - verifier
   - planner
   - implementer
2. Give each role a minimal default tool set.
   Suggested defaults:
   - explorer: `project_list`, `glob`, `grep`, `file_read`, optional read-only
     `bash`
   - verifier: read/search plus validation `bash`
   - planner: read/search/plan/todo, no edit/write
   - implementer: read/search/edit/write/validation
3. Make the `agent` tool description list only roles currently available under
   the route and permission context.
4. Keep user/session overrides.
   The escape hatch should remain explicit: debug/full profile, config rule, or
   user-approved allowed tool list.
5. Make recursive delegation hard to trigger.
   Subagents should not see `agent`/`swarm` unless explicitly configured.

Likely files:

- `src/tools/agent_tool/mod.rs`
- `src/agent/`
- `src/engine/conversation_loop/mod.rs`
- `src/permissions/mod.rs`
- `src/engine/intent_router.rs`

Acceptance:

- A simple codebase exploration agent cannot edit files.
- A verification agent cannot start broad implementation work.
- The main model sees fewer delegation options unless the user asks for
  delegation.

Implementation notes:

- Added built-in `default` and `planner` agent profiles alongside existing
  `explorer`, `verifier`, and `implementer`.
- Built-in profile tool surfaces are now role-scoped:
  - explorer/planner/verifier cannot use `file_edit` or `file_write`
  - implementer can use edit/write/validation tools
  - no built-in profile exposes recursive `agent` or `swarm`
- AgentTool now assigns a default tool allowlist from the selected profile,
  role, or template when the caller does not provide `allowed_tools`. This
  closes the previous unlimited-subagent default.
- AgentTool contract now lists the built-in profiles and keeps explicit
  `allowed_tools` as the escape hatch for narrow custom tasks.

Validation:

- `cargo test -q agent_tool`
- `cargo test -q permissions`
- `cargo test -q route_scoped_tools`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q agent_tool` - passed, 9 tests.
- `cargo test -q profiles` - passed, 2 tests.
- `cargo test -q permissions` - passed, 46 tests.
- `cargo test -q route_scoped_tools` - passed, 4 tests.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.

### Phase 13 - Skill and memory context gating

Status: completed on 2026-05-08.

Purpose: prevent auxiliary context from becoming hidden instructions.

Tasks:

1. Keep retrieval context fenced as background.
   Preserve the existing "not user instruction text" wording.
2. Gate skill exposure by permission and route relevance.
   The model should not see every skill list on turns that do not need skills.
3. Add a compact skill discovery summary:
   - skill name
   - one-line description
   - when to load it
   Full skill content should enter context only after an explicit tool call or
   route match.
4. Add memory/retrieval budget reporting to runtime diet:
   - memory snapshot chars/tokens
   - retrieval items
   - skill list chars/tokens
5. Add tests for prompt injection boundaries:
   - memory content cannot override workspace instructions
   - retrieval context is not treated as user instruction
   - stale memory is not injected when route does not ask for memory/project
     context

Likely files:

- `src/engine/retrieval_context.rs`
- `src/memory/manager.rs`
- `src/skills/`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/trace.rs`

Acceptance:

- Memory, retrieval, and skills remain useful context, not hidden workflow
  commands.
- A simple local request does not receive large unrelated memory/skill context.

Implementation notes:

- Memory snapshots now include explicit `<memory-instructions>` stating that
  memory is background context, not user instruction text, and cannot override
  the current user request, project instructions, permissions, or runtime
  safety rules.
- Legacy relevant-memory prefetch blocks are now fenced with
  `<relevant-memory-instructions>` using the same "not user instruction text"
  boundary.
- Memory snapshot and memory retrieval injection are now gated by
  `RetrievalPolicy::allows_memory_context()`. Light/Web/None routes do not
  receive stale memory context; Memory/Project/Full routes can still use it.
- Skill full-content injection is fenced as background guidance. Skill listing
  now returns compact discovery summaries only: name, one-line description, and
  when to load.
- Route-scoped tool tests now include skill tools and verify normal code-change
  routes do not expose `skills_list`, `skill_view`, or `skill_manage`.
- Runtime diet trace reports now include memory snapshot chars/tokens,
  retrieval item/token counts, and skill discovery summary chars/tokens.

Validation:

- `cargo test -q retrieval_context`
- `cargo test -q memory`
- `cargo test -q skills`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q retrieval_context` - passed, 9 tests.
- `cargo test -q memory` - passed, 93 tests.
- `cargo test -q skills` - passed, 13 tests.
- `cargo test -q runtime_diet` - passed, 6 tests.
- `cargo test -q route_scoped_tools` - passed, 5 tests.
- `cargo test -q trace` - passed, 6 tests.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.
- `git diff --check` - passed.
- `cargo test -q` - passed, 1088 tests.

### Phase 14 - Closeout and workflow final decoupling

Status: completed on 2026-05-08.

Purpose: keep evidence internal unless the user, risk level, or failure state
needs it surfaced.

Tasks:

1. Separate "runtime evidence record" from "user-facing closeout".
   Runtime should always record validation/evidence state. The final answer
   should stay natural by default.
2. Default low-risk successful code changes to concise or hidden closeout.
   Full `Closeout:` should appear only when:
   - validation failed or was partial
   - risk is high
   - user requested details
   - debug/live-eval mode asks for full evidence
3. Consider changing low-risk `require_final_closeout` to internal-only.
   Do not remove trace evidence; only stop forcing a user-visible completion
   block.
4. Add tests for:
   - low-risk success: no full `Closeout:`
   - not verified: concise caveat
   - validation failure: full actionable evidence
   - live eval/debug env: full closeout preserved

Likely files:

- `src/engine/code_change_workflow.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/trace.rs`
- `scripts/run_live_eval.sh`
- `scripts/live_eval_report_parser.py`

Acceptance:

- Normal completed coding turns read like an assistant response, not a workflow
  report.
- `/trace`, live eval summaries, and learning events still preserve structured
  evidence.

Implementation notes:

- `WorkflowCloseout` now carries route risk so visibility decisions can keep
  high-risk successful work in full structured closeout while allowing ordinary
  work to stay concise.
- Default visibility is now:
  - `Full` for high-risk, failed, or partial closeouts
  - `Concise` for passed low/medium-risk closeouts without real residual risk
  - `Concise` for low/medium-risk `not_verified` closeouts when there is no
    pending or rejected acceptance review
  - `Full` when acceptance is pending/rejected or env forces full
- Concise `not_verified` output now says `Done with caveats` and names the
  missing verification without emitting a full `Closeout:` block.
- `PRIORITY_AGENT_CLOSEOUT_VISIBILITY=full` remains the explicit escape hatch;
  the live-eval script already defaults this env value to `full`, preserving
  scorer-visible structured closeout.

Validation:

- `cargo test -q closeout`
- `cargo test -q trace`
- `python3 -m py_compile scripts/live_eval_report_parser.py`
- `bash -n scripts/run_live_eval.sh`

Validated on 2026-05-08:

- `cargo test -q closeout` - passed, 11 tests.
- `cargo test -q code_change_workflow` - passed, 14 tests.
- `cargo test -q trace` - passed, 6 tests.
- `python3 -m py_compile scripts/live_eval_report_parser.py` - passed.
- `bash -n scripts/run_live_eval.sh` - passed.
- `cargo check -q` - passed.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo test -q` - passed, 1090 tests.

### Phase 15 - Runtime diet gates and sample-turn baselines

Status: completed on 2026-05-08.

Purpose: make "less over-control" measurable.

Tasks:

1. Define route-level budgets:
   - direct answer: 0 tools when no tools are needed
   - scoped file mutation: <= 4 visible tools
   - code creation/edit: target <= 12 visible tools
   - planning/research/delegation: route-specific caps
   - prompt-visible instruction text: target <= 2,500 tokens for common turns
2. Add deterministic sample prompts:
   - `帮我把这个文件删了吧`
   - `帮我做一个贪吃蛇游戏吧，用 python 做吧`
   - `我在运行中发现了一个问题，你帮我看看是怎么回事吧`
   - `帮我对比 claude 和 opencode 的 agent 指令设计`
3. Record expected runtime diet labels for each sample:
   - prompt token band
   - exposed tool count
   - workflow context label
   - closeout visibility
   - validation evidence state
4. Make budget failures actionable.
   Test failures should print which layer or tool profile caused bloat.

Likely files:

- `src/engine/trace.rs`
- `src/engine/prompt_context.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/tui/slash_handler/learning.rs`
- possibly a small `scripts/runtime_diet_smoke.sh`

Acceptance:

- We can prove a normal turn is lightweight before and after changes.
- Future additions that bloat context or tool exposure fail tests or show a
  clear `/quick` warning.

Implementation notes:

- Added deterministic runtime-diet sample prompts for direct answer, scoped
  file delete, Python code creation, running-issue debugging, and Claude versus
  opencode instruction-design comparison.
- Added route-level tool exposure budget tests with actionable failure output
  showing prompt label, route, reason, exposed tools, and budget.
- Added common-turn prompt budget tests using the current base prompt plus
  root `AGENTS.md` runtime-guidance section; the budget is 2500 estimated
  tokens for common turns.
- Split `CodeChange` and `BugFix` route tool allowlists. Normal code creation
  now stays within a 12-tool surface, while bug-fix routes keep LSP/symbol
  tools within their own 12-tool budget.
- Tightened delegation routing so a bare `agent` subject, such as "agent
  instruction design", no longer triggers delegation. Explicit delegate,
  subagent, parallel, swarm, or Chinese delegation signals still do.

Validation:

- `cargo test -q runtime_diet`
- `cargo test -q route_scoped_tools`
- `cargo test -q prompt_context`
- `cargo check -q`

Validated on 2026-05-08:

- `cargo test -q runtime_diet` - passed, 6 tests.
- `cargo test -q route_scoped_tools` - passed, 4 tests.
- `cargo test -q prompt_context` - passed, 5 tests.
- `cargo test -q intent_router` - passed, 13 tests.
- `cargo fmt --check` - passed.
- `cargo check -q` - passed.
- `cargo test -q` - passed, 1074 tests.

## Next Execution Order After Phase 14

Follow-up implementation phases in this plan are complete. Next work should be
chosen from validation gaps found during live use, release-hardening gates, or a
new reviewed plan.

The completed plan now has project-instruction diet, explicit loading
semantics, route/sample measurement gates, core tool contracts, role-scoped
subagent surfaces, gated auxiliary context, and concise default closeout.
Future work should use these gates to catch prompt or tool-surface bloat while
changing more surfaces.

## Original Execution Order (Completed Through Phase 8)

1. Phase 0: add regression cases and measurement first.
2. Phase 3: fix natural-language routing because it affects which policy runs.
3. Phase 4: harden evidence semantics so correctness no longer depends on
   prompt compliance.
4. Phase 5: simplify user-facing closeout.
5. Phase 2: shrink route-scoped tool exposure after routing is reliable.
6. Phase 1: diet the base prompt once runtime checks cover the removed rules.
7. Phase 6: implement destructive scope contract.
8. Phase 7 and 8: isolate legacy workflow and add ongoing bloat measurement.

The order intentionally moves runtime correctness before prompt deletion. That
keeps behavior safe while reducing model burden.

## First Batch To Implement

Status: completed on 2026-05-08.

Batch 1 should be small enough to validate quickly:

1. Add route tests for Chinese natural creation requests. Done.
2. Update `IntentRouter` to classify `做/写/创建/生成 + language/object` as
   `CodeChange`.
   Done.
3. Add closeout/evidence tests proving empty validation does not mean passed.
   Done.
4. Add concise-closeout tests that fail under current verbose behavior. Done.

Expected validation:

```bash
cargo fmt --check
cargo test -q intent_router
cargo test -q auto_verify
cargo test -q code_change_workflow
cargo check -q
```

Actual validation for Batch 1:

```bash
cargo fmt --check
cargo test -q intent_router
cargo test -q closeout
cargo test -q auto_verify
cargo clippy -q -- -D warnings
cargo test -q
cargo check --all-features -q
bash -n scripts/run_live_eval.sh
```

Result: `cargo test -q` passed with `1042 passed; 0 failed`.

## Rollback Switches

- `PRIORITY_AGENT_TOOL_PROFILE=full` should continue exposing the broader tool
  surface when needed.
- `PRIORITY_AGENT_WORKFLOW_CONTRACT=0` should continue disabling model-led
  workflow contract calls.
- `PRIORITY_AGENT_WORKFLOW_ENABLED=1` should remain available for legacy
  workflow eval paths until those are retired or isolated.
- `PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=1` is the explicit legacy workflow
  switch; the older workflow env remains compatibility-only.
- `PRIORITY_AGENT_CLOSEOUT_VISIBILITY=hidden|concise|full` should remain the
  user/debug escape hatch for closeout verbosity.
- `PRIORITY_AGENT_ROUTE_SCOPED_TOOLS=0` and
  `PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE=1` should continue bypassing route-scoped
  tool filtering during diagnosis.
- If section-based instruction loading changes behavior too aggressively, keep a
  temporary env fallback such as `PRIORITY_AGENT_AGENTS_MD_FULL=1` or equivalent
  before removing prefix-based loading.

## Success Criteria

The optimization is successful when:

1. Simple tasks use fewer prompt tokens and fewer visible tools.
2. The model still has enough freedom to solve the task naturally.
3. Destructive and validation safety are enforced by runtime checks.
4. Normal final answers are short and task-focused.
5. `/trace` retains the full evidence trail for debugging and evals.
6. Live evals distinguish model reasoning failures from runtime framework
   interference.
7. `AGENTS.md` contains high-signal repo guidance rather than product history or
   workflow doctrine.
8. Instruction loading can include long documentation files without injecting
   their entire prefix into normal turns.
9. Role, permission, tool, memory, and skill surfaces are route-aware and visible
   in runtime diet reports.

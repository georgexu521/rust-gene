# Agent Tools And Permissions Alignment Plan

Date: 2026-05-24

Source note: `/Users/georgexu/Downloads/06-agent_tools_permissions_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch after
`86c2b741 Implement runtime spine diagnostics`.

## 0. Conclusion

This note is useful for `priority-agent`. It describes exactly the next
runtime-hardening layer after the current runtime spine work: once the model has
context and proposes a tool action, the product needs one clear action-review
pipeline that decides whether the action exists, is well-formed, is worth doing,
is within scope, is safe, needs user confirmation, needs a checkpoint, and
produces a structured observation.

The project already has a strong foundation:

- `src/tools/mod.rs` defines a `Tool` trait, tool schemas, result metadata,
  operation kinds, registry profiles, and availability hooks.
- `src/permissions/mod.rs` has permission modes, sourced allow/deny/ask rules,
  risk classification, command-specific rule matching, trusted workspace and
  domain checks, and explainable permission decisions.
- `src/engine/conversation_loop/permission_controller.rs` turns permission
  checks into user approval requests, trace events, recovery guidance, and
  desktop metadata.
- `src/engine/conversation_loop/tool_execution_controller.rs` enforces exposed
  tools, parameter validation, resource limits, destructive scope, action
  checkpoint restrictions, permission requests, hooks, trace, and tool result
  metadata.
- `src/engine/action_decision.rs` implements runtime-owned action scoring.
- `src/engine/task_context.rs`, `src/engine/context_ledger.rs`, and
  `src/engine/evidence_ledger.rs` already turn tool results into task state,
  context facts, verification proof, and closeout evidence.
- `src/engine/checkpoint.rs` and `src/tools/rewind_tool/` give the project a
  checkpoint/rewind base.

The main gap is not missing pieces. The main gap is fragmentation. The checks
from the note are spread across `Tool`, `PermissionContext`,
`PermissionController`, `ToolExecutionGate`, file tools, bash classification,
destructive scope, resource policy, and result normalization. That makes the
system capable, but not yet as clear as the note's ideal:

```text
parse_action
  -> validate_args
  -> worth_doing?
  -> check_permission
  -> ask_user / deny / revise
  -> checkpoint
  -> run_tool
  -> observation
  -> state
```

The next plan should therefore create a single visible action-review contract
without deleting the existing specialized checks.

## 1. Point-By-Point Alignment

| Note point | Current project state | Gap | Plan |
| --- | --- | --- | --- |
| 0. LLM proposes; runtime must check before executing | Mostly implemented. The execution controller checks exposure, args, resource policy, destructive scope, permissions, hooks, and runtime metadata. | The checks do not produce one canonical decision object. | P0: add `ActionReview` as the shared decision record. |
| 1. Tool system gives the agent hands | Implemented. Tool registry includes file, search, bash, git, diff, format, memory, task, agent, MCP, worktree, web, and desktop-related tools. | Tool catalog exists, but product-facing capability tiers are not summarized from one contract. | P0: add `ToolContract` summary derived from each tool. |
| 2. Do not expose every tool | Implemented. `ToolExposurePlan` scopes tools by task stage and action checkpoint. Permission mode also controls whether tools should be exposed. | Exposure is mostly name/stage based. It cannot always reason from per-call risk, output size, network, checkpoint need, or side effect class. | P2/P3: make exposure consume `ToolContract` and route policy. |
| 3. Tool calls must be structured | Implemented through provider tool schemas and `Tool::validate_params`. | Validation is shallow: required fields and primitive types, but weak coverage for enum, bounds, array item schema, object shape, oneOf/anyOf, path patterns, and unknown keys. Invalid actions become tool errors rather than a clean `revise` decision. | P1: structured schema validator and `revise` result. |
| 4. Tool definition should include schema, risk, confirmation | Partially implemented. `ToolSchema`, `ToolPermissionLevel`, `ToolOperationKind`, output schema, retry/idempotency, defer/load flags, UI render kind, and `requires_confirmation` exist. | Risk and safety are partly inferred by tool name, bash classifier, permission context, file guardrails, and individual tool code. There is no single machine-readable contract per invocation. | P0: canonical `ToolContract` and per-invocation `ToolActionProfile`. |
| 5. Permission system decides whether action can execute | Implemented with `PermissionContext`, rule sources, modes, risk levels, `PermissionController`, approval channel, and trace. | `PermissionDecision` has `Allow/Deny/Ask` only. The note's `revise` case is represented indirectly by schema errors or unexposed tool errors. | P1: introduce action-review decisions `allow`, `deny`, `ask_user`, `revise`. |
| 6.1 Tool exists | Implemented by registry lookup and exposed-tool check. | Unknown/unexposed tool feedback is not a typed revise event with available alternatives. | P1: typed `tool_not_available` and `tool_not_exposed` review results. |
| 6.2 Args valid | Implemented at basic level by `Tool::validate_params`. | Needs deeper JSON-schema subset and better repair hints. | P1: validator upgrade and tests for representative tools. |
| 6.3 Path inside workspace | Partially implemented. Permission context checks trusted workspace roots; file tools have path identity and high-risk target checks; bash detects external absolute paths. | Path policy is duplicated and can drift between file, bash, permission, and MCP paths. | P3: shared `WorkspaceBoundaryPolicy`. |
| 6.4 Destructive operation | Strong partial implementation. Bash danger classifier, file target guardrails, destructive scope contract, git push/high-risk checks, and action checkpoint restrictions exist. | Destructive classification is not attached as one stable fact to every action review; checkpoint policy is not uniformly derived from destructiveness. | P3/P4: `SideEffectClass` plus checkpoint policy. |
| 6.5 Network access | Partially implemented. Bash classifier detects network access; `web_fetch`/`web_search` and trusted-domain checks exist. | Network policy is not a first-class route/permission budget. Package install, curl/wget/git clone, MCP, plugin, GitHub, and web tools should share one network-side-effect vocabulary. | P3: `NetworkPolicy` with explicit allowlist and confirmation reason. |
| 6.6 External side effects | Partially implemented. Git push, remote tools, plugin run, MCP tools, memory clear, and some GitHub operations are high/medium risk. | No shared external-side-effect taxonomy for "local mutation" vs "remote mutation" vs "credential/auth" vs "publication/deploy". | P3: add `ExternalSideEffect` classification and trace metadata. |
| 6.7 Budget limits | Implemented in several places: `ResourcePolicy.max_tool_calls`, bash timeout, output truncation/artifacts, file size limits, context budget, compaction. | The note's budget check is not one permission/action-review decision. File read size, output size, command duration, tool count, and parallelism are not reported as a single budget verdict. | P5: `ActionBudgetVerdict` and budget-exceeded trace/report assertions. |
| 6.8 Current task scope | Partially strong. `AgentTaskState.allowed_scope`, goal drift checks, destructive scope, action checkpoint, and phase-aware tools all help. | Scope verdict is spread across goal drift and destructive scope, and it is not always visible as a first-class permission reason or desktop decision. | P2/P3: include scope verdict in action review and permission metadata. |
| 7. Permission result has allow/deny/ask_user/revise | Partially implemented. Allow/ask/deny exist; revise is implicit. | Need explicit revise so the model can correct malformed or unavailable actions without treating them as execution failures. | P1: decision enum and model-facing revise observation. |
| 8. Permission pseudocode | Implemented as multiple controllers rather than one function. | Harder to test the complete order and failure priority. | P0/P1: pure evaluator tests for the full decision order. |
| 9. Checkpoint before mutation | Implemented for file mutation paths and rewind tooling. | Needs explicit policy for format, git mutations, shell write redirection, migrations, and generated-file refusal. The user-visible checkpoint id should be tied to the action review. | P4: checkpoint policy per action. |
| 10. Tool returns observation | Implemented through `ToolResult`, result metadata, normalizer, evidence ledger, context ledger, task-state observations, and runtime diagnostics. | Observation shapes differ by tool. Some tools still return mostly text; `output_schema` coverage is partial. | P5: normalized `ToolObservation` with per-tool adapters. |
| 11. Full tool flow | The project has every stage, but not one named flow. | Need one testable "action review -> execution -> observation -> state" spine. | P6: integration regression. |
| 12. Tool granularity | Good mixed strategy. Fine-grained file/search/diff/format tools plus bash for flexible shell. | Bash remains the largest risk surface; more common shell tasks can be promoted into narrow tools or command families. | P7: promote safe command families and tighten bash. |
| 13. Project feature: action scoring | Implemented in `ActionDecision`, attached to results and trace. | Scores currently inform diagnostics more than execution. No explicit "low value, revise to inspect first" gate yet. | P2: worth-doing gate with conservative thresholds. |
| 14. Keep scoring and permission separate | Partially good. `ActionDecision` and `PermissionContext` are separate. | Tool execution currently combines their outputs late. Need the final review to show both verdicts side by side. | P0: `ActionReview { worth, permission, checkpoint, budget }`. |
| 15. First policy | Mostly covered in pieces. Workspace-only, network, auto allow, blocked patterns, confirmation, timeout, file limits, and checkpoint all exist somewhere. | Need a single policy surface users can inspect and desktop can display. | P8: `action_review policy explain` and desktop panel. |
| 16. Core graph | Current runtime graph matches the note conceptually. | Missing canonical trace event for the whole graph. | P6: `action.review` trace event and live-eval assertions. |
| 17. Overall agent architecture | Well aligned after runtime-spine work. | Next maturity step is action-review productization, not more prompt text. | All phases. |

## 2. Development Plan

### P0 - Canonical Action Review Contract

Goal: make the runtime produce one structured review for every tool call before
execution.

Add:

- `src/engine/action_review.rs`
- `ActionReviewDecision`: `Allow`, `AskUser`, `Deny`, `Revise`
- `ActionReviewReason`: `tool_not_available`, `tool_not_exposed`,
  `invalid_arguments`, `low_value_action`, `permission_required`,
  `permission_denied`, `path_outside_workspace`, `destructive_scope_violation`,
  `network_requires_confirmation`, `external_side_effect_requires_confirmation`,
  `budget_exceeded`, `checkpoint_required`, `safe_to_execute`
- `ActionReview`:
  - tool name and call id;
  - final decision;
  - score verdict from `ActionDecision`;
  - permission verdict from `PermissionContext`;
  - scope verdict from goal/destructive checks;
  - budget verdict from `ResourcePolicy`;
  - checkpoint verdict;
  - user-facing reason;
  - model-facing recovery instruction;
  - trace/debug metadata.

Integration:

- Keep existing checks in `ToolExecutionGate` and `PermissionController`.
- Add a pure review builder that calls those components or mirrors their
  read-only facts.
- Attach `action_review` metadata to every denied, permission-requested, and
  executed tool result.
- Record `TraceEvent::ActionReviewed`.

Validation:

```bash
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q permission_controller -- --test-threads=1
```

### P1 - Explicit Revise Path

Goal: malformed or unavailable actions should ask the model to revise the action,
not appear as ordinary execution failures.

Work:

- Add `revise` handling for:
  - unknown tool;
  - tool not exposed in this request;
  - invalid params;
  - wrong tool for a target, such as raw file mutation on notebooks;
  - action checkpoint asking for broad bash mutation before a patch tool.
- Model-facing message should be concise:

```text
Action rejected before execution: invalid_arguments.
Choose one of the exposed tools and provide valid arguments.
```

- UI/trace should show that no side effect happened.
- Live eval should classify repeated malformed tool calls as `agent_flow`
  unless the model corrects itself.

Validation:

```bash
cargo test -q direct_task_behavior -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### P2 - Worth-Doing Gate

Goal: use `ActionDecision` to prevent low-value or phase-misaligned actions from
becoming noisy tool loops, while avoiding over-control.

Rules for v1:

- Do not block ordinary reads/searches unless repeated or obviously off-scope.
- Revise high-risk edits when:
  - stage is `Understand` and no relevant observation exists;
  - score says low uncertainty reduction and high risk;
  - action would mutate before any scoped read on code-like files.
- Ask user, not revise, when the action might be valid but exceeds the user's
  scope.
- Always include a lower-risk suggested next action.

Validation:

```bash
cargo test -q action_decision -- --test-threads=1
cargo test -q stop_check -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
```

### P3 - Shared Boundary And Side-Effect Policy

Goal: stop duplicating path/network/side-effect reasoning across permission,
file tools, bash tools, MCP, plugins, git, remote, and desktop.

Add:

- `WorkspaceBoundaryPolicy`
  - normalizes relative/absolute paths;
  - checks trusted workspace roots;
  - labels system paths, home-private paths, generated/dependency paths,
    repository metadata, notebooks, credentials, and external directories.
- `NetworkPolicy`
  - local-only;
  - trusted domains;
  - package install;
  - unknown network command;
  - remote service mutation.
- `ExternalSideEffect`
  - none;
  - local_workspace_mutation;
  - local_machine_mutation;
  - network_read;
  - network_write;
  - git_remote_publication;
  - database_or_deploy;
  - credential_or_auth;
  - plugin_or_mcp_unknown.

Use these in:

- `PermissionContext::risk_level`
- bash command classification metadata
- file tool high-risk diagnostics
- `ActionReview`
- desktop permission metadata

Validation:

```bash
cargo test -q permissions -- --test-threads=1
cargo test -q file_tool -- --test-threads=1
cargo test -q command_classifier -- --test-threads=1
```

### P4 - Checkpoint Policy Before Mutation

Goal: checkpoint behavior should be explicit, not just tool-specific.

Add `CheckpointPolicy` to action review:

- `NotNeeded`
- `RequiredAndPresent`
- `RequiredButMissing`
- `Unavailable`

Rules:

- File write/edit/patch: required.
- Format tool: required if it can modify files.
- Bash write redirection or known file mutation: ask user or require checkpoint
  unless it is a known safe validation artifact path.
- Git checkout/reset/clean/branch deletion: ask user and record checkpoint or
  explicit non-file rollback limitation.
- Migrations/database/deploy: ask user; checkpoint local files but do not claim
  external rollback unless a tool provides it.

Validation:

```bash
cargo test -q checkpoint -- --test-threads=1
cargo test -q rewind -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
```

### P5 - Unified Observation Object

Goal: every tool result should produce a compact observation that can feed the
model, state, trace, desktop, and evals consistently.

Add `ToolObservation`:

- status: `success`, `failed`, `denied`, `revised`, `cancelled`, `timed_out`;
- summary;
- files read;
- files changed;
- command run;
- validation result;
- permission decision;
- checkpoint id;
- artifact path;
- state updates;
- recommended next action.

Implementation:

- Keep `ToolResult` for compatibility.
- Add adapters in `tool_result_controller.rs` or a new
  `tool_observation.rs`.
- Store observation in result metadata and context ledger.
- Render observation in `<task-state>` recent observations.

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
```

### P6 - Runtime Trace And Eval Assertions

Goal: live evals should prove the full action review graph exists.

Add runtime-spine assertions for:

- malformed action -> revise;
- outside workspace -> deny or ask user, no execution;
- network command -> ask user unless trusted;
- destructive command -> ask user or destructive-scope block;
- low-value edit before read -> revise;
- mutation -> checkpoint metadata present;
- successful tool -> observation recorded into task state.

Files:

- `scripts/live_eval_report_parser.py`
- `scripts/run_live_eval.sh`
- representative `evalsets/live_tasks/*.yaml`

Validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
bash scripts/live-eval-summary-smoke.sh
```

### P7 - Tool Granularity Cleanup

Goal: keep bash flexible, but reduce routine risk by promoting common safe
families into narrower semantics.

Candidates:

- `run_tests` facade over safe validation command families;
- `start_dev_server` / background task facade with explicit terminal task
  contract;
- `install_dependencies` facade that always asks or follows trusted package
  policy;
- `git_diff`/`git_status` facade separate from mutating `git`;
- `format_code` facade already exists, but should publish checkpoint policy.

This does not require removing `bash`. It makes the common safe path easier for
the model to choose and easier for the runtime to review.

### P8 - Desktop And CLI Explainability

Goal: the user should see why an action was allowed, revised, denied, or sent
for approval.

Desktop:

- Permission card shows:
  - decision kind;
  - risk/side-effect class;
  - path/network/scope reason;
  - checkpoint status;
  - exact allowed-always rule that would be persisted;
  - safer alternative if denied/revised.
- Trace drawer shows `Action review` before `Tool execution`.

CLI/TUI:

- `/permissions explain` includes action-review fields.
- `/trace` summary includes action-review counts:
  - allowed;
  - ask_user;
  - denied;
  - revised;
  - checkpoint_required.

Validation:

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop test:ui-smoke
cargo test -q permission -- --test-threads=1
```

## 3. Recommended First Implementation Batch

Start with the narrowest product-safe slice:

1. Add `src/engine/action_review.rs` with data types and pure tests.
2. Build reviews from existing facts: exposed-tool set, registry lookup,
   `Tool::validate_params`, `ActionDecision`, `PermissionContext`, resource
   policy, destructive scope, and checkpoint availability.
3. Wire `ToolExecutionGate` denied paths to emit `ActionReview` metadata.
4. Add `TraceEvent::ActionReviewed` and one runtime-spine behavior test.
5. Do not change default allow/ask/deny behavior yet except for explicitly typed
   `revise` metadata on unknown/unexposed/invalid actions.

This first batch makes the architecture visible without taking risky behavior
changes.

## 4. What Not To Do

- Do not replace the existing permission system. It already has valuable
  rule-source, mode, trusted-root, command, remote, MCP, and desktop metadata.
- Do not move all safety into prompts. The note is clear that runtime checks
  must carry hard constraints.
- Do not block normal low-risk reads/searches just because the action score is
  imperfect. The worth-doing gate should mainly prevent risky premature writes
  and loops.
- Do not make desktop UI the source of truth. Desktop should display
  `ActionReview`; runtime should own it.
- Do not make a broad generic sandbox the first step. The current project can
  gain more reliability by unifying action review before adding heavier
  isolation.

## 5. Completion Criteria

This plan is complete when:

- every tool call produces an `ActionReview`;
- action-review decisions include `allow`, `ask_user`, `deny`, and `revise`;
- unknown/unexposed/invalid tool calls are classified as `revise`, not generic
  tool failures;
- path, network, destructive, external side-effect, budget, and scope verdicts
  appear in trace metadata;
- mutations have explicit checkpoint policy metadata;
- tool observations are normalized enough for task state, desktop, and evals;
- live-eval/report assertions cover the main permission failure modes;
- desktop and CLI surfaces show why a risky action was blocked or asked.

## 6. Implementation Progress

### Batch 1 - P0/P1 Visible Action Review Spine

Status: implemented on 2026-05-24.

Code changes:

- Added `src/engine/action_review.rs` with the canonical review data model:
  `ActionReviewDecision`, `ActionReviewReason`, `ToolContractReview`,
  worth/permission/scope/budget/checkpoint verdicts, and pure builder tests.
- Added `TraceEvent::ActionReviewed` with `action.review` trace labeling and
  control-loop classification.
- Wired `ToolExecutionGate` to build one review before execution, attach
  `action_review` metadata to denied, pre-executed, read-only, and read-write
  tool results, and record `action.review` trace events.
- Classified unknown, unexposed, and invalid-argument tool calls as typed
  `revise` reviews while preserving existing execution/denial behavior.
- Preserved current approval semantics: `ask_user` is metadata/trace for
  actions that still flow through `PermissionController`.

Validated:

```bash
cargo fmt --check
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q permission_controller -- --test-threads=1
```

Next recommended batch:

- P1 schema validation depth: enum/bounds/object/array/unknown-key checks.
- P2 worth-doing gate in metadata first, then conservative revise behavior for
  high-risk edits before any relevant read.
- P3 shared boundary/side-effect taxonomy for path, network, remote, MCP, and
  bash classification.

### Batch 2 - P1 Structured Parameter Validation

Status: implemented on 2026-05-24.

Code changes:

- Upgraded the default `Tool::validate_params` implementation from shallow
  top-level required/type checks to a reusable JSON Schema subset validator.
- Added support for:
  - root object type checks;
  - nested `required` fields;
  - `type` arrays such as `["string", "null"]`;
  - `enum` and `const`;
  - `anyOf` and `oneOf`;
  - nested object properties;
  - `additionalProperties: false`;
  - array `items`, tuple item schemas, `minItems`, and `maxItems`;
  - string `minLength` and `maxLength`;
  - numeric `minimum`, `maximum`, `exclusiveMinimum`, and
    `exclusiveMaximum`.
- Preserved the existing top-level missing-required message shape, such as
  `Missing required parameter: command`, so old recovery paths stay stable.
- The stricter unknown-key behavior only triggers when the tool schema
  explicitly sets `additionalProperties: false`.

Validated:

```bash
cargo fmt
cargo test -q validate_params -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

Next recommended batch:

- P2 worth-doing gate as conservative metadata first, then a narrow revise rule
  for risky edits before relevant read evidence.
- P3 shared boundary/side-effect taxonomy so path/network/remote/MCP/bash
  decisions use common labels.

### Batch 3 - P2 Conservative Worth-Doing Gate

Status: implemented on 2026-05-24.

Code changes:

- Extended `ActionReview` worth verdicts with:
  - `has_relevant_observation`;
  - `premature_mutation`;
  - `suggested_next_action`.
- Connected `AgentTaskState` to action review so the review can see recent
  observations, completed steps, and active files.
- Added a narrow `revise` gate for premature code-like mutations:
  - stage is `Understand`;
  - action mutates the workspace;
  - target path is code/config-like;
  - no relevant observation exists for that target;
  - action score is high-risk and low uncertainty-reduction.
- Kept ordinary reads/searches and edits with relevant read evidence unblocked.
- Wired the gate into `ToolExecutionGate` so premature mutations are rejected
  before execution with `primary_reason=low_value_action`.

Validated:

```bash
cargo fmt
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

Next recommended batch:

- P3 shared boundary and side-effect policy: path/workspace labels, network
  labels, remote/external side effects, and trace metadata.

### Batch 4 - P3 Shared Boundary And Side-Effect Vocabulary

Status: implemented on 2026-05-24.

Code changes:

- Added `src/engine/action_policy.rs` as a shared action-boundary vocabulary.
- Added `ActionSideEffectProfile` with:
  - workspace path verdicts;
  - network policy verdict;
  - external side-effect class;
  - local workspace mutation flag;
  - local machine mutation flag;
  - remote side-effect flag.
- Added path labels:
  - `workspace`;
  - `external`;
  - `system`;
  - `home_private`;
  - `repo_metadata`;
  - `dependency`;
  - `generated`;
  - `credential`;
  - `unknown`.
- Added network labels:
  - `none`;
  - `localhost`;
  - `trusted_domain`;
  - `untrusted_domain`;
  - `package_install`;
  - `unknown_network_command`;
  - `remote_service`.
- Added external side-effect labels:
  - `none`;
  - `local_workspace_mutation`;
  - `local_machine_mutation`;
  - `network_read`;
  - `network_write`;
  - `git_remote_publication`;
  - `database_or_deploy`;
  - `credential_or_auth`;
  - `plugin_or_mcp_unknown`.
- Wired the profile into `ActionReview` metadata and `TraceEvent::ActionReviewed`
  with `network` and `external_effect` fields.
- Kept this batch descriptive only; permission decisions still use the existing
  permission controller until the next policy-unification batch.

Validated:

```bash
cargo fmt
cargo test -q action_policy -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

Next recommended batch:

- P4 checkpoint policy before mutation: expose required/tool-managed/missing
  checkpoint state more explicitly, starting with format/bash/git mutation
  metadata.

### Batch 5 - P4 Checkpoint Policy Before Mutation

Status: implemented on 2026-05-24.

Code changes:

- Expanded `ActionReview.checkpoint` into a stable checkpoint-policy verdict:
  - `status`;
  - `enforcement`;
  - `rollback_scope`;
  - `checkpoint_id`;
  - `requires_user_approval`.
- Added explicit checkpoint status labels:
  - `not_needed`;
  - `required_and_present`;
  - `required_but_missing`;
  - `unavailable`.
- Classified file mutation tools as `required_and_present` because they already
  create rollback checkpoints before writes.
- Classified `format` by action:
  - `check` is `not_needed`;
  - `format` is `required_but_missing` until the format tool publishes a
    rollback checkpoint contract.
- Classified shell workspace mutations, including write redirection, as
  `required_but_missing`.
- Classified git mutations by rollback scope:
  - read-only git actions are `not_needed`;
  - local repository metadata/worktree mutations are `unavailable` with
    `rollback_scope=repository_metadata`;
  - `git push` is `unavailable` with `rollback_scope=remote`.
- Tightened side-effect classification so `format action=check` no longer
  reports a local workspace mutation.
- Updated tool-result metadata attachment so an observed file-tool
  `checkpoint.id` is copied into `action_review.checkpoint.checkpoint_id`.

Validated:

```bash
cargo fmt
cargo test -q action_review -- --test-threads=1
cargo test -q action_policy -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q checkpoint -- --test-threads=1
cargo test -q rewind -- --test-threads=1
```

Next recommended batch:

- P5 unified observation object: normalize every tool result into one compact
  observation object for model context, trace, desktop, and eval reporting.

### Batch 6 - P5 Unified Observation Object

Status: implemented on 2026-05-24.

Code changes:

- Added `ToolObservation` to the tool-result normalization path with:
  - `status`: `success`, `failed`, `denied`, `revised`, `cancelled`,
    `timed_out`;
  - compact summary;
  - files read;
  - files changed;
  - command run;
  - validation result;
  - permission decision;
  - checkpoint id;
  - artifact path;
  - state updates;
  - recommended next action.
- Kept `ToolResult` as the compatibility surface and added observation as
  metadata instead of replacing existing result data.
- Wired observation metadata into `ToolRuntimeContext::attach_action_decision`
  so successful, failed, denied, revised, pre-executed, and stream-completed
  results share the same observation object.
- Added `tool_observation` to normalized structured metadata and preserved
  existing `tool_summary` for desktop compatibility.
- Added `context_ledger.tool_observation` as a durable compact observation
  entry.
- Taught `AgentTaskState::observe_tool_context_evidence` to record
  `tool_observation` summaries in `<task-state>` recent observations and mark
  files read/changed as active files.

Validated:

```bash
cargo fmt
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
```

Next recommended batch:

- P6 runtime trace and eval assertions: assert malformed/revised, denied
  side-effect, checkpoint metadata, and successful observation signals in the
  runtime spine and live-eval report path.

### Batch 7 - P6 Runtime Trace And Eval Assertions

Status: implemented on 2026-05-24.

Code changes:

- Added `TraceEvent::ToolObservationRecorded` as a runtime-spine state-update
  signal.
- Recorded `tool_observation_recorded` after tool results receive normalized
  observation metadata, covering denied, revised, pre-executed, read-only, and
  read-write tool paths.
- Extended runtime-spine behavior tests to assert:
  - file mutations expose `checkpoint.status=required_and_present`;
  - tool observations update task-state recent observations;
  - trace summaries include action-review and tool-observation events.
- Extended live-eval runtime-spine assertions with:
  - `event:action_reviewed`;
  - `event:tool_observation_recorded`;
  - `special:action_review_revise`;
  - `special:action_review_deny`;
  - `special:checkpoint_metadata`;
  - `special:tool_observation`.
- Updated `scripts/live_eval_report_parser.py` so action review contributes to
  the decision phase and tool observation contributes to the state-update phase.

Validated:

```bash
cargo fmt
cargo test -q runtime_spine_behavior -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
bash scripts/live-eval-summary-smoke.sh
```

Next recommended batch:

- P7 tool granularity cleanup: continue shrinking broad tool surfaces and
  prefer narrowly scoped file, search, git, and shell affordances where the
  runtime can enforce stronger contracts.

### Batch 8 - P7 Tool Granularity Cleanup

Status: implemented on 2026-05-24 for the first validation facade.

Code changes:

- Added `run_tests`, a strict-schema validation facade over safe local command
  families such as `cargo test`, `cargo check`, `cargo clippy`,
  `cargo fmt --check`, `pytest`, `python -m py_compile`, `go test`,
  `bash -n`, and trusted local assertion scripts.
- Reused the existing bash command classifier so `run_tests` rejects mutation,
  install, network, external-path, write-redirection, and fail-closed shell
  commands before execution.
- Registered `run_tests` in the default tool registry without removing
  `bash`.
- Classified `run_tests` as a non-mutating validation action for action
  decision scoring, tool metadata, normalized tool observations, and context
  ledger validation entries.
- Exposed `run_tests` on code-change and bug-fix routes, but only during
  validate/repair phases or focused-repair validation after a patch is allowed.
  Understand/edit phases continue to hide validation shell tools.

Validated:

```bash
cargo fmt
cargo test -q run_tests_tool -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q tool_metadata -- --test-threads=1
cargo test -q validate_params -- --test-threads=1
```

Next recommended batch:

- Continue P7 by adding the next narrow facade with clear runtime-owned
  semantics, likely read-only `git_status`/`git_diff` or a dev-server/background
  task facade. Prefer the git read-only slice first because it can reduce
  routine `git`/`bash` ambiguity without introducing long-running process
  lifecycle risk.

### Batch 9 - P7 Read-Only Git Facades

Status: implemented on 2026-05-24.

Code changes:

- Added `git_status` as a strict-schema, read-only facade for
  `git status --short` with optional path filtering.
- Added `git_diff` as a strict-schema, read-only facade for `git diff` with
  optional path, range, cached, and stat filtering.
- Kept mutating repository operations on the existing `git` tool; the new
  facades never stage, commit, checkout, branch, or push.
- Added range validation to `git_diff` so flag-like ranges and shell
  metacharacters are rejected before invoking git.
- Registered both facades in the default tool registry.
- Classified both facades as read-only validation evidence for action
  decisions, tool summaries, tool observations, context-ledger diff evidence,
  route-scoped tool exposure, focused repair validation after changes, and
  validate/repair/closeout phases.
- Updated build/review agent-mode recommended tools to prefer the narrower
  validation/git-read affordances without removing `bash`, `diff`, or `git`.

Validated:

```bash
cargo fmt
cargo test -q git_read_tool -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q tool_metadata -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q validate_params -- --test-threads=1
cargo test -q tool_reliability -- --test-threads=1
```

Next recommended batch:

- Continue P7 with a long-running process facade only after checking the
  existing bash background task lifecycle. Candidate: `start_dev_server` with
  explicit background task metadata, stop command, port/url capture, and
  desktop-visible task state.

### Batch 10 - P7 Dev-Server Background Facade

Status: implemented on 2026-05-24.

Code changes:

- Added `start_dev_server`, a strict-schema facade for local dev-server
  commands.
- Reused the existing `bash mode=background` lifecycle instead of creating a
  parallel task manager, so started servers use the existing:
  - `terminal_task` metadata;
  - `bash_output` read path;
  - `bash_cancel` stop path;
  - `bash_tasks` listing path.
- Restricted accepted commands to the existing bash classifier's
  `DevServer` category, and rejected remote network commands, external-path
  access, compound shell control flow, write redirection, fail-closed parse
  results, and declared mutation indicators.
- Added `dev_server_task.v1` metadata with command, handle/task id, status,
  timeout, expected URL, read tool, cancel tool, and classifier details.
- Classified `start_dev_server` as a non-workspace-mutating terminal task in
  action decisions and as a local-machine side effect in side-effect policy.
- Exposed `start_dev_server` on code-change and bug-fix routes, but only in
  validate/repair phases or focused-repair validation after changes are
  allowed.
- Added `run_tests`, `git_status`, `git_diff`, and `start_dev_server` to the
  release tool reliability audit surface with representative samples.

Validated:

```bash
cargo fmt
cargo test -q start_dev_server_tool -- --test-threads=1
cargo test -q action_policy -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q tool_metadata -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q tool_reliability -- --test-threads=1
cargo test -q validate_params -- --test-threads=1
cargo test -q run_tests_tool -- --test-threads=1
cargo test -q git_read_tool -- --test-threads=1
cargo test -q background_shell -- --test-threads=1
cargo test -q agent_mode -- --test-threads=1
```

Next recommended batch:

- Continue P7 with install/package-policy granularity only if we can keep it
  conservative: `install_dependencies` should require explicit package-manager
  intent, expose package manager, manifest path, network classification, and
  approval expectations instead of becoming a broad shell alias.

### Batch 11 - P7 Dependency Install Facade

Status: implemented on 2026-05-24.

Code changes:

- Added `install_dependencies`, a strict-schema package-manager facade for:
  - `npm`, `pnpm`, `yarn`;
  - `python_pip`, `uv_pip`;
  - `cargo fetch`;
  - `go mod download`.
- Avoided arbitrary shell input. The tool builds argv from structured
  parameters: manager, action, packages, dev dependency flag, requirements
  file, working directory, lockfile policy, and timeout.
- Marked the tool as open-world/network and always requiring confirmation
  because dependency installation can download and execute external content.
- Rejected unsafe package names before execution.
- Rejected `cargo add` and `go get` style package additions because they mutate
  manifests; those should be deliberate file edits rather than an install
  facade side effect.
- Added `dependency_install.v1` metadata with generated command, manager,
  args, manifest path, lockfile policy, network class, and approval
  expectation.
- Classified the action as a package-install network/local-machine side effect.
- Exposed the tool on code-change and bug-fix routes, but only in repair phase;
  validate/focused-repair validation do not expose dependency installation.
- Added the tool to release reliability audit samples.

Validated:

```bash
cargo fmt
cargo test -q install_dependencies_tool -- --test-threads=1
cargo test -q action_policy -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q tool_metadata -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q tool_reliability -- --test-threads=1
cargo test -q validate_params -- --test-threads=1
cargo test -q agent_mode -- --test-threads=1
cargo test -q start_dev_server_tool -- --test-threads=1
cargo test -q run_tests_tool -- --test-threads=1
cargo test -q git_read_tool -- --test-threads=1
```

Next recommended batch:

- P7 is now useful enough at the tool-granularity layer. Move to P8 desktop/CLI
  explainability, starting with the CLI/TUI side because the Rust action-review
  metadata is now present: add `/permissions explain` or trace summaries that
  surface decision kind, side-effect class, checkpoint status, and safer
  alternatives.

### Batch 12 - P8 CLI Permission Explainability

Status: implemented on 2026-05-24.

Code changes:

- Extended `/permissions explain` to accept optional JSON params:
  `/permissions explain <tool_name> [json_params]`.
- Kept the existing explainable permission output, but now feeds the same
  params into permission risk classification instead of explaining only a null
  call.
- Reused the active engine permission mode and session-scoped permission rules
  when present, so CLI explanations match the current runtime session more
  closely.
- Added a runtime `ActionReview` preview block to `/permissions explain`,
  showing:
  - final decision and primary reason;
  - tool contract availability/exposure/schema validation;
  - worth scores and preview task stage;
  - permission verdict, risk, confidence, and warnings;
  - side-effect class, network class, path count, and local/remote mutation
    flags;
  - checkpoint status and model recovery instruction.
- Added parser and slash-command tests for JSON params and runtime review
  output.

Validated:

```bash
cargo fmt
cargo test -q permissions -- --test-threads=1
cargo test -q action_review -- --test-threads=1
```

Next recommended batch:

- Continue P8 by adding `/trace` summary aggregation for `ActionReview`
  decisions: allowed, ask_user, denied, revised, and checkpoint_required. That
  should reuse the existing trace event payloads rather than creating another
  action-review parser.

### Batch 13 - P8 Trace Action-Review Aggregates

Status: implemented on 2026-05-24.

Code changes:

- Added `ActionReviewTraceSummary` in `src/engine/trace.rs`.
- `/trace last` now includes an `Action Reviews:` line when the turn contains
  `action.review` events.
- The summary aggregates:
  - total action reviews;
  - `allow`;
  - `ask_user`;
  - `denied`;
  - `revised`;
  - `checkpoint_required`;
  - latest reviewed tool/decision/reason.
- Counted checkpoint-required reviews from explicit `checkpoint_required`
  reasons and checkpoint statuses that imply required/unavailable rollback.
- Added trace-summary regression coverage to keep the new aggregate visible in
  `/trace` output.

Validated:

```bash
cargo fmt
cargo test -q trace_summary -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo fmt --check
git diff --check
```

Next recommended batch:

- Continue P8 on the desktop/permission-card side if this repo contains the
  desktop app in scope; otherwise keep productizing CLI surfaces by adding a
  compact `/trace recent` action-review signal and a live-eval/report assertion
  that fails if risky tool runs lack an `action.review` event.

### Batch 14 - P8 Desktop Permission Action Review

Status: implemented on 2026-05-24.

Code changes:

- Added `action_review` metadata to desktop-facing permission request events.
  The Tauri desktop stream now receives the same review object that the Rust
  execution gate records before asking for user approval.
- Extended desktop permission timeline summaries to render:
  - action-review decision;
  - primary reason;
  - risk;
  - side-effect class;
  - network class;
  - checkpoint status;
  - checkpoint user-approval requirement;
  - scope and budget blocks;
  - recovery guidance.
- Updated web-preview and native-smoke permission fixtures with representative
  `action_review` payloads so desktop UI tests exercise the real display path.
- Added run-state regression coverage for permission requests that carry
  action-review facts.
- Verified the desktop preview in the browser: the permission card shows
  `review ask_user`, `network requires confirmation`, remote publication, and
  checkpoint status before approval.

Validated:

```bash
cargo fmt
cargo test -q permission_controller -- --test-threads=1
cargo test -q desktop_runtime -- --test-threads=1
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Next recommended batch:

- Finish P8 with a compact `/trace recent` action-review signal and a
  live-eval/report assertion that fails if risky tool runs lack an
  `action.review` event.

### Batch 15 - P8 Trace Recent And Risky Tool Review Assertion

Status: implemented on 2026-05-24.

Code changes:

- `/trace recent` now includes a compact `action_reviews=` signal whenever a
  turn contains `action.review` events.
- The compact signal exposes total reviews, allow/ask/deny/revise counts,
  checkpoint-required count, and latest tool/decision/reason.
- Added a runtime-spine assertion alias:
  `special:risky_tool_action_review`.
- Added sample-facing aliases such as `risky_tool_action_review`,
  `risky_tools_action_review`, and `action_review_for_risky_tools`.
- The live-eval parser now detects risky tool executions from trace or
  top-level agent events and reports:
  - `risky_tool_runs`;
  - `risky_tool_reviewed`;
  - `risky_tool_missing_action_review`.
- When `special:risky_tool_action_review` is required, the report fails the
  runtime-spine assertion if any risky tool execution lacks a matching
  `action.review` call id.
- Live-eval task reports and aggregate summaries now include the risky-tool
  review counts.

Validated:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
cargo test -q trace_summary -- --test-threads=1
cargo test -q trace_recent_line -- --test-threads=1
cargo fmt --check
```

Additional parser assertion smoke:

```bash
python3 - <<'PY'
from scripts.live_eval_report_parser import runtime_spine_metrics_from_events
missing = runtime_spine_metrics_from_events(
    [{"event": "trace_summary", "trace": {"events": [
        {"type": "tool_started", "tool": "bash", "call_id": "call_bash_1"}
    ]}}],
    assertions=["special:risky_tool_action_review"],
)
assert missing["runtime_spine_status"] == "failed"
reviewed = runtime_spine_metrics_from_events(
    [{"event": "trace_summary", "trace": {"events": [
        {"type": "action_reviewed", "tool": "bash", "call_id": "call_bash_1"},
        {"type": "tool_started", "tool": "bash", "call_id": "call_bash_1"},
    ]}}],
    assertions=["special:risky_tool_action_review"],
)
assert reviewed["runtime_spine_status"] == "passed"
PY
```

Next recommended batch:

- Continue into the remaining tools/permissions plan items that are not part of
  P8, starting with the next highest-risk gap in the checklist rather than
  adding more trace surfaces.

### Batch 16 - P7/P8 Permission Risk Sync For Narrow Facades

Status: implemented on 2026-05-24.

Code changes:

- Updated `PermissionContext::risk_level` so the narrow tools added during P7
  are not treated as unknown low-risk tools:
  - `run_tests`, `git_status`, `git_diff`, `list_mcp_resources`, and
    `read_mcp_resource` are explicit low-risk read/validation tools.
  - `start_dev_server` is explicit medium risk because it starts a local
    long-running process.
  - `install_dependencies`, `mcp_auth`, `plugin_manage`, and `plugin_runtime`
    are explicit high-risk tools.
- Fixed the important AutoAll gap where `install_dependencies` could otherwise
  be auto-approved despite the tool's own confirmation contract.
- Extended permission explanations with concrete warnings for:
  - package installs;
  - local dev-server processes;
  - MCP auth flows;
  - plugin side effects.
- Added an action-review regression proving `install_dependencies` becomes
  `ask_user` with `network_requires_confirmation` under AutoAll.

Validated:

```bash
cargo test -q permissions -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo fmt
cargo fmt --check
cargo test -q package_install_permission_explanation -- --test-threads=1
cargo test -q dependency_install_is_typed_ask_user_in_auto_all -- --test-threads=1
```

Next recommended batch:

- Sweep the same permission-risk sync for other deferred or plugin/MCP tools,
  especially where a tool's concrete name differs from older generic risk keys
  such as `plugin` or `mcp`.

### Batch 17 - Permission Risk Baseline For Side-Effect Tools

Status: implemented on 2026-05-24.

Code changes:

- Extended `PermissionContext::risk_level` beyond the first narrow-facade sync
  to cover additional default and full-profile side-effect tools:
  - `format action=format` is high risk until checkpoint enforcement is
    tool-published; `format action=check` remains low risk.
  - notebook cell mutations are high risk; notebook reads remain low risk.
  - `config action=set`, `skill_manage` create/patch/delete, `rewind`,
    `powershell action=execute`, and cron create/remove/run are high risk.
  - task writes, task output append, todo writes, swarm spawn/execute/clear,
    desktop/browser, send_message/share/copy are medium risk.
- Added permission explanation warnings for format mutation, config mutation,
  skill mutation, notebook mutation, and rewind mutation.
- Added an AutoAll baseline test that locks down which side-effect tools must
  still ask and which read/check variants remain auto-allowed.

Validated:

```bash
cargo fmt
cargo test -q auto_all_permission_risk_baseline_for_side_effect_tools -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo fmt --check
git diff --check
```

Next recommended batch:

- Connect these richer permission risk labels back into route-scoped exposure,
  so high-risk side-effect tools are less likely to be exposed in early
  understand/planning phases unless the route explicitly needs them.

### Batch 18 - Route-Scoped Dependency Install Exposure

Status: implemented on 2026-05-24.

Code changes:

- Added an explicit `IntentRoute::dependency_install_intent` signal so
  dependency installation is represented as a route fact, not inferred later
  from a broad CodeChange/BugFix workflow.
- Updated the intent router to recommend `install_dependencies` only for
  explicit package/dependency installation prompts, including:
  - package-manager commands such as `pip install`, `npm install`, `pnpm i`,
    `yarn add`, `cargo add`, and `go get`;
  - direct English/Chinese dependency phrases such as "install dependencies",
    "add package", "安装依赖", "安装包", and "补依赖";
  - explicit install follow-ups in package-manager/interpreter contexts, while
    preserving "only report / 只报告 / 不要安装" as non-install intent.
- Removed `install_dependencies` from the unconditional CodeChange and BugFix
  route allowlists.
- Filtered learned or mode-injected `install_dependencies` recommendations out
  of route-scoped exposure unless `dependency_install_intent` is true.
- Adjusted build agent mode so it no longer adds dependency installation to
  generic build routes, but preserves it when the original prompt explicitly
  asked for dependency installation.
- Updated route-scoped tool tests so generic Python creation and generic
  debugging do not expose `install_dependencies`, while explicit dependency
  install prompts do.

Validated:

```bash
cargo fmt
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q intent_router -- --test-threads=1
cargo test -q agent_mode -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Continue tightening route exposure for the remaining high-risk side-effect
  families that still enter broad routes too early, especially config/MCP/plugin
  mutation and notebook/format/rewind tools.

### Batch 19 - Route-Scoped MCP Auth Exposure

Status: implemented on 2026-05-24.

Code changes:

- Added an explicit `IntentRoute::mcp_auth_intent` signal so MCP authentication
  is only routed as an auth/token action when the user asks for OAuth,
  authorization, login, token, credential, or Chinese equivalents such as
  "认证", "授权", "登录", "令牌", or "凭据".
- Changed configuration route recommendations:
  - generic MCP/config prompts still get `config` and `mcp`;
  - explicit MCP auth prompts additionally get `mcp_auth`.
- Removed `mcp_auth` from the unconditional Configuration route allowlist.
- Filtered learned `mcp_auth` recommendations out unless
  `mcp_auth_intent` is true, preventing a previous successful auth from
  re-opening auth in unrelated MCP config turns.
- Added route-scoped tests proving generic `mcp 配置` hides `mcp_auth`, while
  explicit OAuth/授权登录 prompts expose it.

Validated:

```bash
cargo fmt
cargo test -q intent_router -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q agent_mode -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Continue the same explicit-intent tightening for broad mutation tools whose
  route exposure still gives the model more write surface than needed, starting
  with `format` and other tools where one name contains both safe read/check
  actions and mutating actions.

### Batch 20 - Parameter-Sensitive Format Tool Contract

Status: implemented on 2026-05-24.

Code changes:

- Tightened `FormatTool` so the runtime can distinguish safe check calls from
  mutating formatter calls at the tool-contract layer:
  - `action=check` now reports `ToolOperationKind::Read`, does not require
    confirmation, and is concurrency-safe.
  - `action=format` now reports `ToolOperationKind::Edit`, requires
    confirmation, is not concurrency-safe, and emits a user-facing confirmation
    prompt.
  - the tool now publishes `ToolPermissionLevel::HighRisk` because one valid
    invocation can rewrite files.
  - the schema is strict and rejects unknown keys through
    `additionalProperties: false`.
- Extended action-review tests so format check/mutation reviews verify the
  tool contract facts, not only the checkpoint verdict.

Validated:

```bash
cargo fmt
cargo test -q format_tool_contract_is_parameter_sensitive -- --test-threads=1
cargo test -q format_check_checkpoint_is_not_needed -- --test-threads=1
cargo test -q format_mutation_checkpoint_is_required_but_missing -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Apply the same parameter-sensitive contract cleanup to other multi-action
  tools, especially `notebook`, `config`, `skill_manage`, and `rewind`, so
  route exposure can remain useful while action review sees precise risk facts.

### Batch 21 - Parameter-Sensitive Config Tool Contract

Status: implemented on 2026-05-24.

Code changes:

- Tightened `ConfigTool` so runtime review can separate read-only config
  inspection from persistent configuration mutation:
  - `action=get`, `schema`, `export`, and `doctor` now report
    `ToolOperationKind::Read`.
  - `action=list` now reports `ToolOperationKind::List`.
  - `action=set` now reports `ToolOperationKind::Write`, requires
    confirmation, is not concurrency-safe, and emits a confirmation prompt for
    the target key.
  - the tool now publishes `ToolPermissionLevel::HighRisk` because one valid
    invocation can mutate persistent agent configuration.
  - the schema is strict and rejects unknown keys through
    `additionalProperties: false`.
- Reused the same contract style as `FormatTool`, keeping route exposure useful
  for read actions while making action review precise for mutation.

Validated:

```bash
cargo fmt
cargo test -q config_tool_contract_is_parameter_sensitive -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Continue parameter-sensitive contracts for mutation-capable multi-action
  tools, with `notebook`, `skill_manage`, and `rewind` as the next likely
  candidates.

### Batch 22 - Parameter-Sensitive Notebook Tool Contract

Status: implemented on 2026-05-24.

Code changes:

- Tightened `NotebookTool` so runtime review can separate notebook inspection
  from notebook cell mutation:
  - `action=read` and `read_cell` now report `ToolOperationKind::Read`, do not
    require confirmation, and are concurrency-safe.
  - `action=edit_cell`, `insert_cell`, and `delete_cell` now report
    `ToolOperationKind::Edit`, require confirmation, are not concurrency-safe,
    and emit a notebook-specific confirmation prompt.
  - the tool now publishes `ToolPermissionLevel::HighRisk` because valid
    invocations can rewrite notebook code/markdown cells.
  - the schema is strict and rejects unknown keys through
    `additionalProperties: false`.
- Added a tool-contract regression for read-vs-edit notebook invocations.

Validated:

```bash
cargo fmt
cargo test -q notebook_tool_contract_is_parameter_sensitive -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Continue parameter-sensitive contracts for the remaining mutation-capable
  administrative tools, especially `skill_manage` and `rewind`.

### Batch 23 - Parameter-Sensitive Skill Manage Contract

Status: implemented on 2026-05-24.

Code changes:

- Tightened `SkillManageTool` so runtime review can separate skill discovery
  and reading from skill file mutation:
  - `action=list` now reports `ToolOperationKind::List`.
  - `action=view` and `reload` now report `ToolOperationKind::Read`.
  - `action=create` now reports `ToolOperationKind::Write`.
  - `action=patch` and `delete` now report `ToolOperationKind::Edit`.
  - create/patch/delete require confirmation, are not concurrency-safe, and
    emit a skill-specific confirmation prompt.
  - the tool now publishes `ToolPermissionLevel::HighRisk` because valid
    invocations can create, overwrite, or delete `SKILL.md` files.
  - the schema is strict and rejects unknown keys through
    `additionalProperties: false`.
- Added a tool-contract regression for view-vs-patch skill management
  invocations.
- During validation, the first rerun failed because the local Rust build cache
  filled the disk (`No space left on device`). Cleared reproducible cargo build
  artifacts with `cargo clean`, which removed 84.5GiB, then reran validation
  successfully.

Validated:

```bash
cargo fmt
cargo test -q skill_manage_contract_is_parameter_sensitive -- --test-threads=1
cargo test -q permissions -- --test-threads=1
df -h .
cargo clean
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Finish the current parameter-sensitive contract sweep with `rewind`, then run
  one broader route/action-review validation pass after the cold build cache is
  warm again.

### Batch 24 - Rewind Tool Contract

Status: implemented on 2026-05-24.

Code changes:

- Tightened `RewindTool` so checkpoint restoration is represented as an
  explicit confirmed file-state edit:
  - all rewind invocations now report `ToolOperationKind::Edit`;
  - all rewind invocations require confirmation and emit a target-aware
    confirmation prompt;
  - the tool now publishes `ToolPermissionLevel::HighRisk`;
  - rewind is never concurrency-safe because it restores file state;
  - the schema is strict and rejects unknown keys through
    `additionalProperties: false`.
- Added a tool-contract regression for path-target rewind.

Validated:

```bash
cargo fmt
cargo test -q rewind_tool_contract_marks_restore_as_confirmed_edit -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo test -q intent_router -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo fmt --check
cargo check -q
git diff --check
```

Next recommended batch:

- Run a broader validation pass over the route/action-review/permission surface,
  then continue with any remaining plan items that still lack an implementation
  batch.

### Batch 25 - Completion Audit And Full Validation

Status: implemented on 2026-05-24.

Plan completion audit:

| Plan item | Status | Evidence | Remaining risk |
| --- | --- | --- | --- |
| P0 Canonical Action Review Contract | Complete for v1 | Batch 1 added `ActionReview`, typed decisions, trace events, and result metadata. | Further product polish can add more policy inputs, but the shared contract exists. |
| P1 Explicit Revise Path | Complete for v1 | Batch 1/2 classify unknown, unexposed, and invalid arguments as `revise`; schema validation is deeper. | More specialized revise hints can be added per tool family. |
| P2 Worth-Doing Gate | Complete for conservative v1 | Batch 3 blocks premature high-risk mutation before relevant read evidence. | The gate is intentionally narrow to avoid over-control. |
| P3 Shared Boundary And Side-Effect Policy | Complete as shared vocabulary; partial as total consolidation | Batch 4 added path/network/external side-effect labels and wired them into action review and traces. | Some specialized file/bash/MCP/plugin checks still own local details; full deduplication is future cleanup, not required for v1. |
| P4 Checkpoint Policy Before Mutation | Complete as review policy; partial as tool-managed rollback coverage | Batch 5 records `not_needed`, `required_and_present`, `required_but_missing`, and `unavailable`. | `format` and raw bash workspace mutation still report `required_but_missing`; actual tool-managed checkpoints for those paths remain future work. |
| P5 Unified Observation Object | Complete for v1 | Batch 6 added `ToolObservation` metadata, context-ledger integration, and task-state recent observations. | More per-tool adapters can improve summaries over time. |
| P6 Runtime Trace And Eval Assertions | Complete for v1 | Batch 7 and Batch 15 added trace/runtime-spine events and live-eval/report assertions, including risky-tool action-review coverage. | More real eval fixtures should be added as product regression coverage grows. |
| P7 Tool Granularity Cleanup | Complete for the planned common facades | Batches 8-11 added `run_tests`, `git_status`, `git_diff`, `start_dev_server`, and `install_dependencies`; Batches 16-24 synced risk, route exposure, and parameter-sensitive contracts. | Bash remains intentionally available for flexible work; additional facades are optional product expansion. |
| P8 Desktop And CLI Explainability | Complete for v1 | Batches 12-15 added `/permissions explain`, `/trace` action-review counts, desktop permission action-review display, and report assertions. | A fuller desktop trace drawer could still be improved, but the user-visible review facts are wired. |

Validation completed:

```bash
cargo fmt
cargo clippy --all-features -- -D warnings
cargo fmt --check
cargo check -q
cargo test -q
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
bash scripts/workflow-production-gates.sh
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
git diff --check
```

Observed validation results:

- Rust full test suite passed: `1817 passed; 0 failed`, plus `3 passed; 0 failed`
  for the binary test target.
- `cargo clippy --all-features -- -D warnings` initially found maintainability
  issues introduced by the new code; fixed them by factoring complex tuples and
  large argument lists into structs, boxing a large enum variant, and simplifying
  a schema validator branch.
- `scripts/live-eval-summary-smoke.sh` passed and generated a smoke summary.
- `scripts/workflow-production-gates.sh` passed all gates.
- Desktop validation passed:
  - TypeScript no-emit check;
  - production build;
  - Playwright UI smoke: `23 passed`.
- Removed validation-only generated noise:
  - deleted `scripts/__pycache__`;
  - restored workflow report timestamp/local-metrics churn generated by the
    workflow gates.

Current conclusion:

- The original P0-P8 plan is complete as a v1 implementation.
- The next practical step is not another feature batch. It is review and commit
  preparation:
  - inspect the large diff by subsystem;
  - decide whether to split commits by runtime, tools, desktop, and docs;
  - commit the validated work.

Optional future hardening after commit:

- Add tool-managed checkpoints for `format action=format`.
- Decide whether bash workspace mutation should be blocked until a checkpoint
  wrapper exists, instead of only surfacing `required_but_missing`.
- Continue consolidating specialized file/bash/MCP/plugin boundary checks into
  the shared `action_policy` vocabulary where that reduces duplication.
- Add dedicated live-eval fixtures for permission/revise/checkpoint regressions.

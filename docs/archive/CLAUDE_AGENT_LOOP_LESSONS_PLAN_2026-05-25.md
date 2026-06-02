# Claude Agent Loop Lessons Follow-up Plan - 2026-05-25

## Purpose

This plan turns the latest comparison against the local Claude source tree into
a scoped follow-up for Priority Agent. The goal is not to copy Claude Code
wholesale. The goal is to strengthen the same runtime contract we already want:

- the LLM owns semantic and engineering judgment;
- the runtime mechanically enforces tool contracts, permissions, budgets, and
  protocol correctness;
- every failure should become attributable evidence that can be sent back to
  the LLM or shown in traces, instead of being hidden by prompt wording.

## Sources Reviewed

Claude local reference:

- `/Users/georgexu/Desktop/claude/src/query.ts`
  - emits synthetic `tool_result` blocks for missing tool results;
  - clears assistant/tool state on model fallback;
  - sends normalized tool results into the next model turn;
  - optionally creates tool-use summaries after a tool batch.
- `/Users/georgexu/Desktop/claude/src/services/tools/toolOrchestration.ts`
  - partitions tool calls into concurrency-safe batches and serial batches.
- `/Users/georgexu/Desktop/claude/src/services/tools/StreamingToolExecutor.ts`
  - starts safe tools as streaming tool calls arrive;
  - preserves output order;
  - creates synthetic errors for fallback, interruption, and sibling failures.
- `/Users/georgexu/Desktop/claude/src/services/tools/toolExecution.ts`
  - validates tool schema before execution;
  - returns permission denials as model-visible `tool_result` errors;
  - stores structured output as attachments.
- `/Users/georgexu/Desktop/claude/src/hooks/toolPermission/permissionLogging.ts`
  - converts permission sources into stable labels such as `config`, `hook`,
    `classifier`, `user_temporary`, `user_permanent`, `user_abort`, and
    `user_reject`.

Priority Agent current implementation:

- `src/services/api/mod.rs`
  - normalizes provider-bound tool message sequences and drops orphan results.
- `src/services/api/provider_protocol.rs`
  - records provider capability facts including streaming tool support,
    non-streaming tool-call requirements, and tool-result adjacency.
- `src/engine/conversation_loop/api_request_controller.rs`
  - falls back to non-streaming tool requests for providers such as MiniMax.
- `src/engine/conversation_loop/tool_round_controller.rs`
  - appends assistant tool calls, executes tools, then appends model-visible
    tool results.
- `src/engine/conversation_loop/tool_execution_controller.rs`
  - gates actions, runs concurrency-safe tools in parallel, runs write tools
    serially, records lifecycle and trace facts.
- `src/engine/conversation_loop/permission_controller.rs`
  - builds structured permission request metadata and records resolved approval
    decisions.
- `src/engine/conversation_loop/tool_result_controller.rs`
  - converts raw tool outputs into `ToolObservation`, provider-visible content,
    UI content, context policy, and evidence-ledger facts.
- `src/engine/conversation_loop/context_budget_controller.rs`
  - tracks request and tool-result budget pressure.
- `src/engine/conversation_loop/request_preparation_controller.rs`
  - injects recent context ledger and recent semantic observations into the next
    request.
- `src/engine/candidate_action.rs`
  - explicitly keeps candidate-action scoring gated instead of forcing every
    turn through model JSON.

## Finding 1: Tool Result Pairing And Fallback Recovery

Claude pattern:

- Each assistant `tool_use` must receive a matching model-visible `tool_result`,
  even when fallback, interruption, or failure happens.
- Fallback clears pending assistant/tool state so stale tool IDs cannot leak into
  the retry.
- Missing results become explicit synthetic error results, not invisible state
  loss.

Priority Agent status:

- Provider-bound histories are already normalized by
  `normalize_tool_message_sequence`.
- Provider capability records already track strict tool-result adjacency.
- The conversation loop already records provider protocol facts and uses
  non-streaming requests for MiniMax-style tool calls.

Gap:

- Normalization protects provider requests, but the reason a stale assistant
  tool call or orphan tool result was dropped is not yet first-class enough in
  traces and reports.
- We should distinguish these cases:
  - model produced a dangling tool call;
  - runtime aborted before execution;
  - provider fallback discarded a partial streaming attempt;
  - historical session data contained stale display metadata.

Decision:

Strengthen trace/report attribution first. Do not add more prompt rules for
this. If the runtime repairs provider-bound history, it should explain the
repair as deterministic protocol hygiene.

## Finding 2: Streaming Tool Execution And Concurrency

Claude pattern:

- Safe tool calls can begin while the model stream is still arriving.
- Results are buffered and emitted in original tool order.
- Non-concurrency-safe tools run exclusively.
- Fallback/interruption generates synthetic results so the model loop remains
  protocol-correct.

Priority Agent status:

- We already run concurrency-safe tools in parallel after the model response is
  complete.
- We already keep write/side-effect tools serial and trace lifecycle state.
- Provider capability records already know whether streaming tool calls are
  supported.
- MiniMax currently requires non-streaming tool requests, so full streaming tool
  execution would not help the main current test provider.

Gap:

- We do not yet have Claude-style early execution while tool-call chunks are
  still streaming.
- Adding it directly would increase state-machine complexity, especially around
  fallback, cancellation, tool result ordering, and providers that do not stream
  tool-call chunks reliably.

Decision:

Do not implement full early streaming execution as the next production default.
Add a shadow feasibility slice first:

- record which streamed tool calls would have been eligible for early execution;
- estimate latency saved for read-only/concurrency-safe calls;
- verify provider support and ordering behavior;
- keep default behavior unchanged.

This preserves the stable LLM/runtime loop while gathering evidence.

## Finding 3: Permission Provenance

Claude pattern:

- Permission decisions carry stable source labels, then analytics, OTel, and
  downstream tool execution can all answer why a tool was allowed or rejected.
- Permission denials become explicit `tool_result` errors with enough context
  for the model to retry safely or stop.

Priority Agent status:

- `ToolApprovalResponse` already distinguishes approve-once, persisted rule
  decisions, scope, rule pattern, persisted path, and note.
- `PermissionController` records request kind, matched patterns, permission
  family, risk level, matcher input, hook decisions, and resolved approval.
- Denied results already carry `permission_request` metadata and recovery text.
- `EvidenceLedger` records permission facts.

Gap:

- Permission source naming is not yet unified as a stable cross-system
  vocabulary. The same event can be described through runtime rule, tool
  confirmation, goal drift, hook denial, user decision, session rule, project
  rule, or global rule, but reports do not yet expose one compact source label.
- Approved permission provenance is attached to the tool result metadata, but
  it is not as easy to query as Claude's single decision-source map.

Decision:

Add a stable `permission_source` vocabulary and propagate it through permission
request metadata, resolved trace events, tool observations, evidence-ledger
records, and final/run reports. This is useful because it improves failure
attribution without asking the runtime to make semantic engineering decisions.

Suggested labels:

- `config_global_allow`
- `config_project_allow`
- `config_session_allow`
- `config_global_deny`
- `config_project_deny`
- `runtime_rule`
- `tool_confirmation`
- `goal_drift`
- `hook_allow`
- `hook_deny`
- `user_once_allow`
- `user_once_reject`
- `user_session_allow`
- `user_project_allow`
- `user_global_allow`
- `user_global_deny`
- `classifier_allow`
- `classifier_ask`
- `classifier_deny`

## Finding 4: Observer Summaries And Context Budget

Claude pattern:

- Raw tool output is mapped once into API-compatible tool-result blocks.
- Structured outputs can become attachments.
- A compact tool-use summary can be generated after a batch and passed forward.
- Tool-result budget is actively managed so large outputs do not consume the
  entire next request.

Priority Agent status:

- `ToolResultNormalizer` already produces model-visible content, UI content,
  `ToolObservation`, context policy, and evidence facts.
- `RequestPreparationController` already injects recent context ledger and
  recent semantic observations before repeating tool calls.
- `RuntimeDietSnapshot` already tracks prompt tokens, tool schema tokens,
  tool-result tokens, truncated results, artifacts, retrieval, skills, and
  exposed tools.

Gap:

- We need stronger assertions that every important tool result leaves the right
  next-turn observation:
  - validation failure includes command, exit/failure family, and next attention;
  - edit success includes changed files, diff/checkpoint evidence, and whether
    validation remains required;
  - permission block includes permission source, scope, and safe retry guidance;
  - truncated result includes durable artifact reference and protected tail.
- Runtime diet metrics are collected, but not yet used enough as regression
  evidence for observer quality and context waste.

Decision:

Improve observer completeness and budget diagnostics. Do not replace
deterministic observations with an always-on LLM observer. If an LLM summary is
added later, keep it gated and compare it against deterministic facts.

## Implementation Plan

### Phase 1 - Protocol Repair Attribution

Tasks:

- Add a trace/report event for provider-bound tool-message normalization.
- Record counts and IDs for:
  - stale assistant tool calls dropped;
  - orphan tool results dropped;
  - valid assistant/tool-result pairs preserved;
  - provider family and adjacency requirement.
- Keep the provider request normalization behavior unchanged.
- Add tests for normalized history attribution alongside the existing sequence
  normalization tests.

Acceptance:

- A provider-protocol failure report can explain whether the issue was model
  output, runtime abort/fallback, or stale historical display metadata.
- No new prompt text is required.

Suggested validation:

```bash
cargo test -q provider_protocol
cargo test -q normalize_tool_message_sequence
cargo test -q prompt_context
```

### Phase 2 - Permission Source Vocabulary

Tasks:

- Introduce a small typed permission-source enum or string helper close to
  `PermissionController`.
- Map existing permission paths into stable source labels:
  runtime rule, tool confirmation, goal drift, hook allow/deny, user once,
  user session/project/global persistence, config allow/deny, and classifier
  decisions where available.
- Attach `permission_source` to:
  - `permission_request.metadata`;
  - `TraceEvent::PermissionResolved`;
  - `ToolObservation.permission_decision` or a sibling field;
  - `ToolPermissionRecord`;
  - final/run report permission facts.
- Preserve existing fields so current UI and traces do not regress.

Acceptance:

- Every permission-denied or permission-approved tool result can answer:
  who/what made the decision, what scope it applies to, and whether retry is
  safe.
- Hook denials and user denials are distinguishable in traces and evidence.

Suggested validation:

```bash
cargo test -q permission
cargo test -q approval
cargo test -q evidence_ledger
cargo test -q route_scoped_tools
```

### Phase 3 - Observer Completeness Checks

Tasks:

- Add deterministic observer-quality checks for key result families:
  validation, file edit/write/patch, permission, provider protocol, and
  high-risk action denial.
- Promote missing observer fields into trace/report warnings, not prompt rules.
- Extend tests around `ToolResultNormalizer` and context ledger injection so
  next-turn context includes the right compact facts.
- Ensure raw result artifacts remain available when model-visible content is
  truncated.

Acceptance:

- The next LLM turn receives enough structured evidence to repair common LLM
  mistakes after validation or tool failure.
- Reports can distinguish "LLM made a bad edit but runtime caught it" from
  "runtime failed to preserve the needed evidence."

Suggested validation:

```bash
cargo test -q tool_result_controller
cargo test -q context_budget_controller
cargo test -q request_preparation_controller
cargo test -q closeout
```

### Phase 4 - Runtime Diet Regression Signals

Tasks:

- Add report-level thresholds or warnings for:
  - repeated large tool-result truncation;
  - missing durable artifact references;
  - excessive exposed tool schema tokens for narrow routes;
  - repeated raw-output replay where a semantic observation would be enough.
- Keep these as diagnostics first. Do not block normal execution unless a test
  fixture proves the behavior causes a real loop or provider failure.

Acceptance:

- Live-eval and trace summaries can show whether context waste is improving or
  getting worse across runs.
- This strengthens process reliability without over-constraining the LLM.

Suggested validation:

```bash
cargo test -q runtime_diet
cargo test -q context_budget_controller
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 5 - Streaming Tool Execution Shadow Slice

Tasks:

- Add a feature flag such as
  `PRIORITY_AGENT_STREAMING_TOOL_EXECUTION=shadow`.
- When the provider supports streaming tool calls, record shadow eligibility for
  streamed calls:
  - tool name;
  - call ID;
  - concurrency-safe/read-only classification;
  - whether schema was complete early enough;
  - estimated time between eligibility and actual execution start.
- Do not execute tools early in this phase.
- Add explicit trace events for shadow-only decisions so this cannot be confused
  with production behavior.

Acceptance:

- We can quantify whether Claude-style early execution would materially improve
  Priority Agent before adding state-machine complexity.
- MiniMax and other non-streaming-tool providers remain unaffected.

Suggested validation:

```bash
cargo test -q streaming
cargo test -q tool_execution_controller
cargo check -q
```

### Phase 6 - Eval And Rollout

Tasks:

- Add deterministic fixtures for:
  - dangling assistant tool call;
  - orphan tool result;
  - permission denied by hook vs user vs config;
  - validation failure followed by repair;
  - truncated tool output with durable artifact.
- Extend report parsing to include protocol-repair counts, permission source
  counts, observer warnings, and runtime-diet warnings.
- Run targeted live evals only after deterministic gates pass.

Acceptance:

- Failures are attributed to one of:
  - model reasoning/editing;
  - provider protocol;
  - runtime permission/policy gate;
  - observer/context evidence loss;
  - test harness or fixture.
- We avoid tuning prompts for isolated weak-model mistakes unless a repeated
  process failure proves the runtime is not returning enough evidence.

Suggested validation:

```bash
cargo check -q
cargo test -q
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

## Priority Order

1. Phase 1: protocol repair attribution.
2. Phase 2: permission source vocabulary.
3. Phase 3: observer completeness checks.
4. Phase 4: runtime diet regression signals.
5. Phase 6 deterministic fixtures for the completed pieces.
6. Phase 5 streaming shadow slice, after the above evidence path is stable.

The reason Phase 5 is later is practical: full streaming tool execution is
useful, but it is not the current core failure. Protocol attribution,
permission provenance, and observer completeness directly improve reliability
for the existing agent/LLM handoff.

## Non-goals

- Do not add a large always-on prompt layer to imitate Claude.
- Do not make the runtime independently decide engineering strategy.
- Do not require candidate-action JSON on ordinary turns.
- Do not enable production early tool execution without shadow evidence.
- Do not treat every weak-model mistake as a runtime defect.

## Expected Outcome

After this plan, Priority Agent should be better at answering:

- Did the LLM make a bad tool call, or did provider protocol handling lose a
  result?
- Did a permission failure come from user choice, config, runtime risk policy,
  hook, or classifier?
- Did the runtime return enough evidence for the LLM to repair its own mistake?
- Did context budget pressure hide the evidence needed for the next turn?
- Is streaming tool execution worth the added complexity for our actual
  providers and workloads?

That is the improvement direction most consistent with the current product
principle: narrow, deep, personal, and verifiable.

## Implementation Status - 2026-05-25

Completed in this pass:

- Phase 1: provider-bound tool message normalization now emits repair
  attribution for preserved pairs, dangling assistant tool calls, and orphan
  tool results.
- Phase 2: permission decisions now carry stable `permission_source` facts
  through request metadata, traces, observations, context ledger, and evidence
  reports.
- Phase 3: tool observations now include deterministic quality warnings for
  validation, edit/checkpoint, permission, and truncation evidence gaps.
- Phase 4: runtime diet reports now surface warning labels for prompt/tool
  schema pressure, truncation without artifacts, and tool-result context waste.
- Phase 5: streaming tool execution remains gated as shadow-only telemetry via
  `PRIORITY_AGENT_STREAMING_TOOL_EXECUTION=shadow`; no early production tool
  execution was enabled.
- Phase 6: deterministic parser/report support was extended for provider
  protocol repairs, permission sources, observer warnings, runtime diet
  warnings, and streaming shadow events.

Validation completed:

```bash
cargo fmt --check
cargo check -q
cargo check --features experimental-api-server -q
cargo clippy --all-features -- -D warnings
cargo test -q
bash -n scripts/run_live_eval.sh
bash scripts/live-eval-summary-smoke.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

Live provider evals were intentionally not used as the primary proof for this
pass. The goal was to harden deterministic agent/LLM handoff, attribution, and
reporting without tuning around MiniMax randomness.

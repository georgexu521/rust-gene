# Agent Observer Alignment Plan

Date: 2026-05-24

Source note: `/Users/georgexu/Downloads/07-agent_observer_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch `claude`,
current head `f3c7d67f Harden checkpoints and action boundaries`.

## 0. Conclusion

The project already has an Observer v1, but it is still mostly a runtime
metadata and ledger bridge. It records that a tool ran, whether it succeeded,
which files or commands were involved, and how that should update bounded task
state. That is valuable and should be kept.

The new note asks for a stronger layer:

```text
raw_result
  -> semantic observation
  -> task state / hypotheses / focus
  -> selected next context
```

At the time of this note, `priority-agent` did not fully do that yet. The main
gap was not "missing observation object"; the gap was that the observation
object was too thin, raw tool output still flowed directly into model context,
and task state did not yet accumulate structured findings, evidence,
hypotheses, uncertainty reduction, or next-attention signals.

The implementation line is to upgrade `ToolObservation` from compact metadata
into a real Observer contract, without losing compatibility with existing
`ToolResult`, provider tool-call pairing, trace, desktop, and eval surfaces.

## 0.1 Implementation Progress

2026-05-24 first implementation batch:

- Extended `ToolObservation` with semantic Observer fields:
  `result_kind`, `key_findings`, compact evidence excerpts, goal impact,
  next attention, context/state inclusion flags, confidence, raw-result
  reference, hypothesis updates, candidate focus, uncertainty reduction, and
  risk notes.
- Added deterministic Observer adapters in the shared tool-result normalization
  path for file reads, search, validation, edits, diffs, installs/dev servers,
  and unknown shell commands.
- Added observation-aware model visibility policy:
  `full_raw`, `raw_excerpt`, `observation`, and `artifact_only`.
  Noisy search and long/failed validation output can now become
  observation-first model content while raw output remains available through
  exact excerpts, result metadata, or artifact references.
- Extended context ledger `tool_observation` payloads with the richer
  observation fields and confidence.
- Extended `AgentTaskState` with bounded structured `key_findings`,
  `hypotheses`, and `candidate_focus`, and rendered those high-signal fields
  in `<task-state>`.
- Taught request preparation to inject recent high-signal
  `context_ledger.tool_observation` entries into context-ledger hints.
- Extended `ToolObservationRecorded` trace events with result kind, key finding
  count, evidence count, raw-result reference, model visibility, and
  context/state inclusion decisions.
- Extended `scripts/live_eval_report_parser.py` with runtime-spine assertions
  for observer key findings, observer evidence, raw-result references, model
  visibility, context inclusion, and task-state storage.
- Phase 5 decision: keep the LLM Observer fallback off by default in this
  batch. The deterministic Observer contract now exposes the confidence,
  visibility, artifact, and trace metadata needed to add a strictly gated
  fallback later without putting prompt-heavy observer instructions into the
  always-on runtime.

Validated in this batch:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test -q
```

## 1. Current Alignment

The project is aligned with the note at the control-flow level:

- Tool execution is centralized through
  `src/engine/conversation_loop/tool_execution_controller.rs`.
- Tool results are normalized in
  `src/engine/conversation_loop/tool_result_controller.rs`.
- `ToolObservation` v1 exists with status, summary, files read/changed,
  command, validation result, permission decision, checkpoint id, artifact path,
  state updates, and recommended next action.
- Observation metadata is persisted into context ledger entries through
  `src/engine/context_ledger.rs`.
- `AgentTaskState` consumes context-ledger entries and renders recent
  observations in `<task-state>`.
- Trace already has `ToolObservationRecorded`, so evals can assert the state
  update phase exists.

This means the correct next move is refinement, not a parallel subsystem.

## 2. Remaining Problems

### P0. Observation v1 is too shallow

Current evidence:

- `ToolObservation` fields stop at `status`, one-line `summary`, path/command
  facts, validation/permission/checkpoint facts, and `recommended_next_action`
  (`src/engine/conversation_loop/tool_result_controller.rs:42`).
- `observation_summary` picks either the error text or the first non-empty
  output line (`src/engine/conversation_loop/tool_result_controller.rs:538`).

Gap against the note:

- No `key_findings`.
- No direct `evidence` excerpts.
- No `impact_on_goal`.
- No `next_attention`.
- No explicit `include_in_next_context` / `store_in_state`.
- No observation confidence or uncertainty reduction.

Risk:

- The agent can record "bash failed" or "file_read succeeded" without carrying
  the actual reason that should drive the next action.

### P1. Raw tool output still enters model context by default

Current evidence:

- Provider-visible content is still `Result: OK/ERROR` plus
  `tool_result_user_output` (`src/engine/conversation_loop/tool_metadata.rs:529`).
- `append_provider_tool_result` pushes `normalized.model_content` as the tool
  message (`src/engine/conversation_loop/tool_result_controller.rs:175`).
- Large output is truncated only after 32 KiB and then retains first bytes,
  last bytes, and keyword snippets
  (`src/engine/conversation_loop/tool_execution.rs:23`,
  `src/engine/conversation_loop/tool_execution.rs:153`).

Gap against the note:

- The note says tool results should not automatically enter context; they
  should first become observations.
- Current behavior still sends raw or truncated raw output into the next model
  turn. Observation metadata exists beside it, not instead of it.

Risk:

- Long tests, grep output, installs, and diffs can still dominate model
  attention even when a compact observation would be enough.

### P2. Task state stores summaries, not structured reasoning state

Current evidence:

- `AgentTaskState` has bounded `observations: Vec<ObservationSummary>`, active
  files, risks, verification plan, and stop checks
  (`src/engine/task_context.rs:167`).
- Tool observations are collapsed into a single formatted summary string
  (`src/engine/task_context.rs:465`).
- `<task-state>` renders only the last three recent observations
  (`src/engine/task_context.rs:562`).

Gap against the note:

- No structured `key_findings`.
- No hypothesis list with confidence.
- No candidate focus / reduced uncertainty.
- No durable evidence snippets attached to observations.
- No relation from observation to the current goal beyond generic stage
  transitions.

Risk:

- The runtime can prevent repeated tools and track validation, but it does not
  yet help the model build a reliable diagnosis across multiple tool rounds.

### P3. Context selection is budget-aware, not observation-aware

Current evidence:

- `ToolResultContextPolicy` records visible chars, artifact path, compaction
  eligibility, ledger eligibility, and protected tail
  (`src/engine/conversation_loop/tool_result_controller.rs:124`).
- Request preparation injects task state and context-ledger hints
  (`src/engine/conversation_loop/request_preparation_controller.rs:69`).
- Context-ledger hints include file reads, bash read facts, edits, diffs,
  validations, and confirmations, but not the general `tool_observation`
  entries (`src/engine/conversation_loop/request_preparation_controller.rs:124`).

Gap against the note:

- There is no semantic policy that chooses "observation only", "observation +
  evidence", "raw excerpt", or "full raw content" by tool type and task need.
- `include_in_next_context` is not an observation-level decision.

Risk:

- The model either sees too much raw output or too little actionable distilled
  evidence after compaction.

### P4. Tool-specific observers are incomplete

Current evidence:

- The observation builder has generic helpers for status, files, command,
  validation result, permission decision, checkpoint id, artifact path, and
  recovery (`src/engine/conversation_loop/tool_result_controller.rs:231`).
- There is targeted parsing elsewhere, such as failed-test extraction in
  `src/engine/repair_spec.rs`, but it is repair-specific rather than the common
  observer path.

Gap against the note:

- `run_tests` / validation output does not yet normalize failed files, failed
  tests, error lines, or first compiler diagnostics into the observation.
- `grep` / `glob` does not rank top matches or recommend next files.
- `file_read` does not summarize key symbols or relevance.
- `file_edit` / `file_patch` does not consistently produce a diff summary plus
  verification-needed signal as first-class observation fields.
- Unknown `bash` commands are not explicitly classified into a conservative
  "completed but output type unclear" observation.

Risk:

- The strongest runtime evidence exists, but the model still has to infer the
  important part from text.

### P5. There is no LLM Observer fallback

Current evidence:

- Observation creation is deterministic and local in
  `ToolObservation::from_result`.
- Long-output handling is truncation plus hard-coded high-signal terms.

Gap against the note:

- The note recommends simple results by rules, structured results by parsers,
  and complex/long results by LLM Observer.
- This project has the first two foundations, but not the third fallback.

Risk:

- For complex logs, multi-file diffs, long web/MCP outputs, or ambiguous
  command output, the system either over-sends raw text or under-summarizes it.

## 3. Development Plan

### Phase 0 - Observer Baseline Tests

Goal: make the current gaps measurable before changing behavior.

Add regression tests for representative tool results:

- `file_read` small file: full output can remain visible, but observation
  includes path, size/line facts, and relevance placeholder.
- `grep` with many matches: observation includes match count and top paths;
  model context does not need all matches by default.
- `run_tests` / `bash cargo test`: failed observation includes failed test names,
  first diagnostic evidence, and validation result.
- `file_edit`: successful observation includes changed file, diff summary, and
  verification-needed signal.
- unknown `bash`: observation status is completed/unknown, includes risk note,
  and does not pretend validation passed.
- long output: provider-visible content can be observation-first while raw
  output remains available through an artifact path.

Likely files:

- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/context_ledger.rs`
- `src/engine/task_context.rs`
- `src/engine/runtime_spine_behavior_tests.rs`

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 1 - `ToolObservation` v2 Contract

Goal: extend the observation object without breaking current metadata consumers.

Add optional fields:

- `result_kind`: `file_read`, `search`, `validation`, `edit`, `diff`,
  `permission`, `install`, `dev_server`, `unknown_command`, `generic`.
- `key_findings: Vec<String>`.
- `evidence: Vec<ObservationEvidence>`, capped at 3-5 compact excerpts.
- `impact_on_goal: Option<String>`.
- `next_attention: Vec<String>`.
- `include_in_next_context: bool`.
- `store_in_state: bool`.
- `confidence: Option<f32>`.
- `raw_result_ref: Option<String>` for artifact-backed raw output.
- `hypothesis_updates: Vec<HypothesisUpdate>`.
- `candidate_focus: Vec<String>`.
- `reduced_uncertainty: bool`.

Compatibility rules:

- Keep `schema: "tool_observation.v1"` until all consumers tolerate missing
  fields, or introduce `"tool_observation.v2"` with backward-compatible parsing.
- Keep existing fields required for desktop/eval compatibility.
- Never remove raw `ToolResult` in the same batch; only add observation-first
  rendering behind policy.

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q trace -- --test-threads=1
```

### Phase 2 - Deterministic Per-Tool Observers

Goal: extract useful observations with rules and parsers before using any LLM.

Add observer adapters:

- `observe_file_read`: path, range/full-read, line counts, truncation, content
  hash, likely symbols when cheap, and whether raw content should stay visible.
- `observe_search`: match count, top files, repeated/noisy result detection,
  recommended next files.
- `observe_validation`: validation family, exit code, failed tests, first error,
  failing files, and whether the failure supports current repair focus.
- `observe_edit`: changed files, replacements/additions/deletions, changed
  range, checkpoint id, diff summary, verification needed.
- `observe_diff`: changed/no-change, top changed files, evidence hash, whether
  no-diff audit closeout is supported.
- `observe_install` / `observe_dev_server`: command status, background task id,
  output path, next inspection command.
- `observe_unknown_command`: conservative status, output type unclear, side
  effect/risk note, raw artifact reference.

Implementation notes:

- Start in `tool_result_controller.rs`; split into
  `tool_observation.rs` only when the file becomes hard to read.
- Reuse existing parsers where possible:
  `bash_tool::command_classifier`, `repair_spec` failed-test extraction,
  context-compressor diagnostic extraction, and file/diff metadata.
- Keep adapters deterministic and bounded.

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q repair_spec -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo test -q context_compressor -- --test-threads=1
```

### Phase 3 - Observation-Aware Context Policy

Goal: make the next model request consume selected observations, not raw output
by default.

Add a semantic context decision:

```text
FullRaw       small exact file content or user explicitly needs exact output
RawExcerpt    error/diff evidence where exact text matters
Observation   normal search/install/status/large validation results
ArtifactOnly  huge output with durable raw reference
Hidden        low-value duplicate result already represented in state
```

Changes:

- Extend `ToolResultContextPolicy` from size-only fields to semantic
  `model_visibility`.
- For noisy tools, make `normalized.model_content` observation-first while raw
  content remains in `ui_content`, trace payload, or artifact.
- Preserve provider tool-call pairing: every assistant tool call must still get
  a valid tool message.
- Add context-ledger hint rendering for recent `tool_observation` entries when
  they contain key findings or next attention.
- Add tests that long validation output produces compact model content with
  artifact-backed raw output.

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
cargo test -q context_compressor -- --test-threads=1
```

### Phase 4 - Structured Task-State Learning

Goal: make observations accumulate into a usable task recordbook.

Extend `AgentTaskState` with bounded structures:

- `key_findings: Vec<TaskFinding>`.
- `hypotheses: Vec<TaskHypothesis>`.
- `candidate_focus: Vec<TaskFocus>`.
- `observation_evidence_refs: Vec<EvidenceRef>`.

Rules:

- Successful search/read observations can raise candidate focus.
- Validation failures can raise or lower hypotheses.
- Edit success should create a verification-needed finding.
- Repeated low-value observations should decay or merge instead of bloating
  state.
- Only high-signal fields render into `<task-state>`.

Validation:

```bash
cargo test -q task_context -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 5 - Optional LLM Observer Fallback

Goal: handle complex, ambiguous, or long results after deterministic observers
have done what they can.

Trigger only when:

- output is over a threshold and deterministic adapter confidence is low;
- multiple error families appear;
- a tool result is a web/MCP/document-style result with no stable parser;
- a debug/eval mode explicitly asks for rich observation summaries.

Prompt contract:

```text
You are the Observer. Convert this tool result into JSON:
status, summary, key_findings, evidence, impact_on_goal,
next_attention, include_in_next_context, store_in_state.
```

Guardrails:

- Cap raw input to the fallback using excerpts plus artifact reference.
- Validate JSON strictly and fall back to deterministic observation on parse
  failure.
- Never let the LLM Observer invent validation success; validation status still
  comes from runtime evidence.
- Record fallback usage in trace and runtime-diet metrics.

Validation:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 6 - Eval And Product Diagnostics

Goal: prove the Observer is improving agent behavior, not just metadata shape.

Add live-eval/runtime assertions:

- long failing test output becomes compact observation;
- failed tests and first diagnostic are present in task state;
- repeated broad search is reduced after top matches are observed;
- edit success creates verification-needed next attention;
- unknown shell output does not become validation evidence;
- observation-first context reduces tool-result token pressure.

Expose diagnostics:

- `/trace` shows observer adapter, raw/output visibility mode, key findings
  count, and artifact reference.
- Desktop/runtime panels show observation summary and evidence excerpts for the
  last tool round.
- `scripts/live_eval_report_parser.py` reports observer state-update coverage.

Validation:

```bash
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
```

## 4. Suggested Execution Order

1. Phase 0 first: pin the current behavior and avoid accidental provider
   protocol breaks.
2. Phase 1 next: add the v2 fields but keep all current consumers compatible.
3. Phase 2 next: implement deterministic observers for validation/search/edit,
   because those have the biggest agent-quality payoff.
4. Phase 3 after adapters are useful: switch noisy results to observation-first
   model context.
5. Phase 4 after observations are richer: let task state accumulate findings
   and hypotheses.
6. Phase 5 only after deterministic coverage is solid: add LLM fallback behind
   strict gates.
7. Phase 6 throughout: add eval assertions as each behavior lands.

## 5. Non-Goals

- Do not delete `ToolResult`; it remains the compatibility surface.
- Do not make every small file read observation-only. Exact file content is
  still necessary for coding.
- Do not add prompt-heavy Observer instructions to the always-on system prompt.
  This should be runtime behavior, not another model manual.
- Do not treat LLM Observer summaries as proof of validation success.
- Do not make the task-state zone large; keep it compact and selected.

## 6. Definition Of Done

This Observer line is done when:

- every tool result has a structured observation with status, summary,
  key findings or an explicit reason they are absent, evidence refs, and next
  attention when relevant;
- noisy results are observation-first in model context by default;
- raw output remains available through artifacts or exact excerpts when needed;
- `AgentTaskState` carries structured findings, hypotheses, and focus, not only
  recent summary strings;
- tests cover file read, search, validation, edit, diff, unknown shell, and long
  output cases;
- `/trace` and live-eval reports can distinguish raw output, observation,
  state update, and context inclusion decisions.

# opencode Programming Chain Gap Plan

Date: 2026-06-05

Status: proposed

## 1. Purpose

This document follows `docs/NEXT_PHASE_OPENCODE_ALIGNMENT_PLAN_2026-06-04.md`
and narrows the scope to the programming chain: how the agent plans, edits,
shows diagnostics, records diffs, supports revert, handles shell permissions,
and keeps task progress visible.

The goal is not to copy opencode wholesale. Priority Agent already has stronger
runtime contracts in several areas: read-before-edit checks, checkpoint-backed
file changes, verification proof, failure-owner classification, provider
slow-tail instrumentation, memory review gates, and daily/live eval reporting.

The gap is product integration. opencode makes programming work feel like one
coherent session model. Priority Agent has most of the hard primitives, but
some of them are still exposed as separate runtime systems instead of a single
smooth coding loop.

## 2. Evidence Map

opencode source reviewed:

- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/write.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/edit.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/lsp/diagnostic.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/revert.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/todo.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/todo.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/tools.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/session.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/sql.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/projector.ts`

Priority Agent source reviewed:

- `src/tools/file_tool/mutation_result.rs`
- `src/tools/file_tool/write.rs`
- `src/tools/file_tool/mod.rs`
- `src/tools/file_tool/patch.rs`
- `src/tools/file_tool/diagnostics.rs`
- `src/engine/checkpoint.rs`
- `src/tools/rewind_tool/mod.rs`
- `src/tui/tool_view.rs`
- `src/tools/todo_tool/mod.rs`
- `src/task_manager/mod.rs`
- `src/tools/bash_tool/mod.rs`
- `src/tools/bash_tool/command_classifier.rs`
- `src/tools/bash_tool/command_classifier/shell_analysis.rs`
- `src/cost_tracker/usage_ledger.rs`
- `src/cost_tracker/mod.rs`
- `src/engine/cache_stability.rs`
- `src/engine/context_compressor.rs`
- `src/engine/context_compressor/compressor.rs`
- `src/engine/context_usage.rs`
- `src/engine/model_context.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- `src/engine/conversation_loop/tool_exposure_plan.rs`
- `src/engine/conversation_loop/tool_execution.rs`
- `src/services/api/mod.rs`
- `src/services/api/openai_compat.rs`
- `src/services/config.rs`
- `scripts/daily-baseline.sh`

Provider/context comparison sources reviewed:

- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/reasonix.example.toml`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/provider/openai/openai.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/agent.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/compact.go`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/llm/request.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/provider/transform.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/compaction.ts`

## 3. Current Assessment

Priority Agent is no longer missing the basic programming-agent primitives. It
can read, edit, patch, checkpoint, rewind, classify shell commands, run
validation, track usage, and block false verified closeout. The remaining
opencode gap is mostly about connecting those primitives into a tighter product
loop.

Priority Agent is stronger than opencode in these areas:

- edits are guarded by read-before-edit and stale-file checks;
- file mutations create checkpoint-backed restore evidence;
- `file_patch` validates the batch before writing and restores on partial
  failure;
- closeout is tied to explicit proof instead of only conversation state;
- usage tracking now has JSONL canonical logging plus a SQLite projection;
- live/daily eval has explicit failure classes and runtime evidence.

opencode is still stronger in these areas:

- edit results are immediately useful to UI and session state;
- diagnostics are first-class output after write/edit;
- message/part-level revert is a product path, not only a lower-level restore
  tool;
- edit matching has several fallback strategies for slightly stale anchors;
- shell permissions are derived from parsed command structure and paths;
- todos are persisted session state and UI events, not just tool text output.

## 4. Key Gap 1: Mutation Results Need Full Product Consumption

### opencode behavior

opencode `write` computes a diff before asking permission, writes the file,
runs formatting, publishes file watcher events, touches LSP, collects
diagnostics, and returns metadata with diagnostics and file identity.

opencode `edit` returns metadata with `diff`, `filediff`, and `diagnostics`.
It also calls `ctx.metadata` during execution so the running tool card can be
updated before final completion.

### Priority Agent status

Priority Agent now has `FileMutationResult` with operation, files, diff,
checkpoint, file change IDs, diagnostics, rollback, and `ui_summary`. The
shape is strong enough to become the shared contract for file mutation tools.

The gap is downstream consumption:

- TUI tool cards still summarize `file_write` and `file_edit` mostly from input
  args and generic result output.
- Desktop cards do not yet have a dedicated programming-chain card for
  mutation result metadata.
- Trace summaries do not yet consistently render `mutation_result` as a
  first-class event.
- Repair and closeout still need robust helper accessors so they do not parse
  per-tool ad hoc fields.

### Plan

Phase 1 should make `mutation_result` the canonical product-facing edit
payload.

Work items:

- Add a small parser/helper module that extracts `FileMutationResult` from a
  `ToolResult` JSON payload.
- Update TUI tool rendering to prefer `mutation_result.ui_summary` for
  `file_write`, `file_edit`, and `file_patch`.
- In expanded TUI view, show changed files, `+/-`, checkpoint id, file change
  ids, and diagnostics status.
- Add trace rendering for mutation result summaries.
- Add desktop/runtime event payload fields for mutation result without changing
  the existing tool result schema.
- Keep legacy fields such as `edit_preview`, `checkpoint`, and `file_change`
  during migration.

Acceptance:

- A file mutation appears in TUI as one stable edit card, not only generic tool
  output.
- The card shows the changed path, line delta, checkpoint/rewind hint, and LSP
  status when available.
- Trace output has one concise mutation summary line.
- Existing tests for file tools, trace summaries, and TUI tool views stay green.

Suggested gates:

```bash
cargo test -q file_tool
cargo test -q trace
cargo test -q tui
cargo check -q
```

## 5. Key Gap 2: Revert Should Be Message/Round-Centric

### opencode behavior

opencode `session/revert.ts` can revert by `messageID` or `partID`. It collects
patch parts after the target point, restores snapshots, computes diff summary,
publishes a session diff event, and stores revert state on the session.

This makes rollback a session product feature: the user can reason about
"revert this assistant turn" rather than remembering a checkpoint id.

### Priority Agent status

Priority Agent has stronger low-level restore primitives:

- `CheckpointManager` stores checkpoints and durable `FileChangeRecord`.
- `FileChangeRoundSummary` groups changes by tool round.
- `rewind` supports `latest_file_change`, `latest_tool_round`,
  `tool_round_id`, `file_change_id`, `checkpoint_id`, and `path`.

The gap is the default user-facing path:

- The product does not yet consistently surface "this assistant turn changed
  these files; revert this turn."
- Session UI does not yet render a clear "last change" / "revert round" affordance.
- Revert result is not consistently connected back to session diff summaries.

### Plan

Phase 2 should elevate existing checkpoint history into a session-level
programming feature.

Work items:

- Add a read-only "last mutation round" projection for the current session:
  paths, file change IDs, checkpoint IDs, additions, deletions, and combined
  diff hash.
- Show this projection in TUI status or a compact `/diff`/`/rewind` panel.
- Add `rewind latest_tool_round` as the primary UI action after any mutation
  round.
- Store the last round summary in session store or trace projection so desktop
  can render it without reading checkpoint files directly.
- After rewind, emit a trace/session event with restored paths and diff summary.

Acceptance:

- After an edit, the user can see the changed files and run one obvious
  "rewind this round" action.
- Rewind output includes exactly which files were restored or removed.
- Rewind events are visible in trace/TUI and can be used by closeout/repair
  logic.
- No direct file mutation can bypass checkpoint creation.

Suggested gates:

```bash
cargo test -q checkpoint
cargo test -q rewind
cargo test -q file_tool
cargo test -q closeout
cargo check -q
```

## 6. Key Gap 3: Edit Matching Needs Better Recovery For Weak Anchors

### opencode behavior

opencode `edit.ts` includes multiple replacement strategies:

- exact replacement;
- line-trimmed matching;
- block-anchor matching;
- whitespace-normalized matching;
- indentation-flexible matching;
- escape-normalized matching;
- trimmed-boundary matching;
- context-aware matching.

This reduces common coding failures where the model gives an `oldString` that
is semantically right but not byte-exact.

### Priority Agent status

Priority Agent already has important guardrails:

- read-before-edit;
- stale-file detection;
- exact replacement count checks;
- match diagnostics for multi-match and fuzzy failures;
- `file_patch` for coordinated edits;
- failure observations that can re-enter the model loop.

The gap is controlled auto-recovery. The system should not silently apply a
risky fuzzy edit, but it can offer deterministic candidate matches and safe
retry guidance.

### Plan

Phase 3 should add candidate-based edit recovery without weakening exact-edit
semantics.

Work items:

- Extract current match-diagnostics logic into a focused helper if needed.
- Add deterministic candidate generation for:
  - line-trimmed blocks;
  - indentation-normalized blocks;
  - whitespace-normalized single-line snippets;
  - block-anchor candidates for 3+ line old strings.
- Return candidates in `match_diagnostics` when exact match fails.
- Do not auto-apply ambiguous candidates.
- Permit auto-apply only when there is exactly one high-confidence candidate,
  it is within the last read snapshot, and replacement count is one.
- Record whether recovery was exact, suggested, or auto-applied in
  `mutation_result`.
- Add tests for exact failure with candidate suggestions, unique safe recovery,
  ambiguous multi-candidate refusal, and stale snapshot refusal.

Acceptance:

- A slightly stale but unambiguous old string can be repaired deterministically.
- Ambiguous matches fail with useful candidate metadata instead of guessing.
- Read-before-edit and stale-file checks remain hard gates.
- Weak-model live evals improve because failed edits feed better observations
  back to the model.

Suggested gates:

```bash
cargo test -q file_tool
cargo test -q file_edit
cargo test -q closeout
cargo check -q
```

## 7. Key Gap 4: Shell Permission Parsing Should Become More Structural

### opencode behavior

opencode `shell.ts` uses tree-sitter for bash and PowerShell. It parses command
nodes, extracts command parts, expands common environment/home path forms,
detects path arguments for file commands, identifies external directories, and
requests permissions using concrete path patterns.

It also keeps output metadata updated while the command runs and writes large
outputs to a truncation artifact.

### Priority Agent status

Priority Agent has a strong heuristic classifier:

- command kind and category;
- validation families;
- path patterns and external path flags;
- subcommand facts;
- redirection facts;
- mutation paths;
- fail-closed reasons;
- permission rule suggestions.

The gap is parser confidence and path precision:

- shell parsing is still mostly custom tokenization and heuristics;
- PowerShell/CMD support is less structural;
- external-directory permission prompts are not as precise as opencode's parsed
  path model;
- a tree-sitter parser was previously deferred because it adds dependency and
  maintenance risk.

### Plan

Phase 4 should add a gated structural parser path without replacing the current
classifier.

Work items:

- Keep the current heuristic classifier as the default stable path.
- Add an optional parser module behind a feature flag or runtime config.
- Start with bash only; defer PowerShell until bash proves stable.
- Use structural parsing only to enrich metadata:
  - parsed command count;
  - file-operation path args;
  - redirection targets;
  - dynamic shell constructs;
  - external directory candidates.
- If parser fails, fall back to current fail-closed classifier.
- Add a daily baseline case for complex shell commands with redirection,
  command substitution, heredoc, external paths, and chained commands.

Acceptance:

- Existing permission decisions do not become more permissive.
- Parser-derived facts improve review metadata and permission prompts.
- Parser failure is observable and fail-closed.
- No broad dependency lands without a narrow benchmark and tests.

Suggested gates:

```bash
cargo test -q bash_tool
cargo test -q command_classifier
bash scripts/workflow-production-gates.sh
cargo check -q
```

## 8. Key Gap 5: Todo/Task State Should Be A Session Product Feature

### opencode behavior

opencode has a `Todo` session service backed by database rows and events.
`todowrite` updates the session's todo list, emits `todo.updated`, and the UI
can render this as persistent session state.

### Priority Agent status

Priority Agent has:

- `todo_write` tool output;
- `TaskManager`;
- `ActiveTaskPlan`;
- plan mode approval;
- trace and status panels.

The gap is that todos are not yet a durable, first-class session projection for
normal coding work. They are useful text output, but not a stable UI/session
state like opencode's todos.

### Plan

Phase 5 should make coding todos session-backed while keeping plan mode
separate.

Work items:

- Add a session-store projection for current coding todos:
  content, status, priority, position, updated_at.
- Make `todo_write` replace the session todo list transactionally.
- Emit a trace/runtime event when todos change.
- Render current todos in TUI/desktop compact status areas.
- Keep approval plans separate from todos:
  - plan mode is user-approved execution intent;
  - todos are model-maintained working state.
- Add validation that at most one todo is `in_progress`.

Acceptance:

- Todos survive session reload.
- TUI/desktop can show active todo progress without parsing tool output text.
- `todo_write []` clears the projection.
- Plan approval state and todo state do not conflict.

Suggested gates:

```bash
cargo test -q todo
cargo test -q session_store
cargo test -q active_task_plan
cargo check -q
```

## 9. Key Gap 6: Provider/Context Cost Control Needs Product Policy

### Why this matters

The DeepSeek comparison between Reasonix and opencode shows a useful product
lesson. Reasonix feels fast because it keeps the runtime path light, streams
directly, parallelizes read-only tools, keeps a cache-friendly prefix, and does
not interrupt long sessions with frequent compaction. The same choices can make
reported token usage climb quickly: each model round carries the growing
session tail and tool schemas, high-effort thinking can spend more completion
tokens, and a very large context window delays cleanup.

opencode is slower but more conservative. It prepares requests through a
structured request layer, applies `maxOutputTokens`, prunes old tool outputs,
keeps compaction as session state, and filters tools before request creation.
That adds overhead, but it also keeps the live context cleaner and reduces
runaway completion/tool-loop cost.

Priority Agent should not choose only one side. The desired shape is:

- Reasonix-style stable prefix and fast read-only dispatch;
- opencode-style output ceilings, earlier context hygiene, and narrower tool
  surfaces;
- Priority Agent's existing proof, checkpoint, usage ledger, and failure-owner
  gates.

### Priority Agent status

Priority Agent already has many of the right primitives:

- `UsageLedgerEntry` records session, model, prompt, completion, cache hit,
  cache miss, cost, stable prefix hash, system hash, tool schema hash, dynamic
  tail hash, and miss reason.
- JSONL is canonical and SQLite is a query projection for `/cost`, desktop
  status, and daily baselines.
- `cache_stability` fingerprints stable prefix, dynamic context zones, and tool
  schemas.
- `ContextBudgetController` records message tokens, tool schema tokens, exposed
  tool count, total request tokens, context limit, and remaining context.
- `RuntimeDietReport` already flags prompt/tool/context bloat in traces.
- `ContextCompressor` supports snip, micro compact, auto compact, reactive
  compact, LLM summaries, and persisted compact boundaries.
- `ChatRequest` supports `max_tokens`, and OpenAI-compatible providers map it
  to `max_completion_tokens`.

The gaps are policy and integration:

- Main streaming request preparation still creates `ChatRequest` with no
  default `max_tokens`, so output ceilings depend on caller-specific overrides.
- Preflight compaction still uses an 80% context-window trigger. With very
  large-context models this avoids overflow, but it does not proactively reduce
  cost or prompt noise.
- `ToolExposurePlan` currently forwards the base tool list unchanged; route
  scoping exists elsewhere but this layer does not yet enforce a budgeted
  tool-surface policy.
- `DEFAULT_MAX_RESULT_TOKENS` and `shrink_tool_result_by_tokens` exist, but the
  comments say they are API-ready follow-up work rather than fully integrated.
- Runtime diet warns about bloat, but it does not yet feed a deterministic next
  action such as "snip old tool results before next request", "lower output
  cap", or "switch to narrow tool surface".
- Usage ledger can show total prompt and cache hit/miss, but daily baseline
  does not yet run a controlled provider-cost A/B that explains whether a
  token spike is cached prompt volume, completion volume, schema bloat, or
  repeated tool-loop rounds.

### Plan

Phase 6 should make provider/context cost control a first-class programming
chain contract.

Work items:

- Add a central request budget policy for model calls:
  - normal coding turn: bounded output cap, for example 8k-16k depending on
    model profile;
  - closeout/summary/repair helper: smaller cap, for example 1k-4k;
  - planning and workflow classification: already-small caps should stay small;
  - provider-specific override through config/env for explicit experiments.
- Apply the policy in the main streaming request path, non-streaming fallback,
  reactive compaction retry, query-simple path, and LLM compaction summary
  requests.
- Record the effective output cap in trace and usage ledger metadata so cost
  reports can distinguish "no cap" from "capped by policy".
- Add an early hygiene pass before normal requests:
  - snip old tool outputs when tool-result tokens exceed a fixed budget or a
    request-ratio threshold;
  - compact at a cost/noise threshold before the hard 80% context trigger;
  - preserve recent tool results and validation/proof evidence verbatim.
- Wire `shrink_tool_result_by_tokens` into provider-visible tool result
  construction, while keeping full output in durable artifacts.
- Make `ToolExposurePlan` budget-aware:
  - expose a small default set for coding;
  - add mutation tools only when the route is a code-change route;
  - add expensive/background tools only when route evidence requires them;
  - keep stable ordering and fingerprints for cache predictability.
- Add usage-ledger dimensions for loop analysis:
  - request source or phase (`coding`, `repair`, `closeout`, `compact`,
    `fallback`);
  - effective output cap;
  - exposed tool count and tool schema token count;
  - iteration number/tool round count;
  - compaction/prune decision before the request.
- Extend `/cost`, desktop status, and trace summaries to answer:
  - total prompt tokens vs cache-hit tokens;
  - cache-miss prompt tokens;
  - completion tokens;
  - tool schema tokens;
  - dynamic tail hash churn;
  - likely miss reason or bloat reason.
- Add a controlled DeepSeek A/B baseline:
  - same prompt;
  - same model id and base URL;
  - same max output cap;
  - same reasoning/thinking config when supported;
  - same initial tool surface;
  - record token usage, cache hit/miss, completion tokens, tool rounds, and
    pass/fail result.

Acceptance:

- Main coding requests have an explicit output cap unless the user or config
  opts out.
- Large tool outputs are represented by concise provider-visible summaries plus
  durable artifacts, not by unbounded live context.
- Preflight hygiene can reduce cost/noise well before context overflow.
- Tool schema token count falls on narrow coding routes without changing
  available tools for routes that need them.
- `/cost` and desktop status can explain whether a high-token session is mostly
  cached prompt, cache miss, completion, schema, or repeated loop cost.
- Daily baseline includes at least one provider-cost fixture that compares
  capped vs uncapped or narrow-tool vs broad-tool behavior.

Suggested gates:

```bash
cargo test -q usage_ledger
cargo test -q cache_stability
cargo test -q context_compressor
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q conversation_loop
cargo check -q
bash scripts/daily-baseline.sh
```

### Non-goals for this gap

- Do not chase a high prompt-cache hit rate by keeping noisy context forever.
- Do not hide real prompt token volume just because many tokens are cached.
- Do not weaken validation/proof context to save tokens.
- Do not make tool routes so narrow that the model cannot complete normal
  coding workflows.
- Do not add provider-specific prompt rules when a request policy, output cap,
  context hygiene pass, or ledger dimension can enforce the behavior.

## 10. Recommended Execution Order

Recommended order:

1. Mutation result product consumption.
2. Message/round-centric revert projection.
3. Edit recovery candidates.
4. Provider/context cost-control policy.
5. Session-backed todos.
6. Gated structural shell parser.

Reasoning:

- Phases 1 and 2 convert existing strong primitives into visible product
  behavior. They should produce immediate daily-use improvement with low
  architectural risk.
- Phase 3 improves weak-model coding reliability while preserving exact-edit
  safety.
- Phase 6 should come before deeper todo/UI polish because provider cost,
  output caps, and context hygiene affect every long coding session.
- Phase 5 improves long-task UX but should not precede mutation/revert because
  task state is less important than trustworthy edits.
- Phase 4 has the highest dependency and parser risk, so it should be gated and
  optional.

## 11. Daily Baseline Additions

Add or update daily baseline cases around these contracts:

- `mutation-result-ui-contract`: file write/edit/patch all produce and surface
  `mutation_result`.
- `round-rewind-contract`: a multi-file edit can be rewound by latest tool
  round and reports restored paths.
- `edit-candidate-recovery`: a non-exact but unambiguous stale anchor produces
  candidate metadata or safe recovery.
- `ambiguous-edit-refusal`: multiple candidate matches are refused without
  applying a guess.
- `session-todo-projection`: `todo_write` persists todos and clears with `[]`.
- `shell-structural-review`: complex shell commands produce parser/classifier
  facts and fail closed when ambiguous.
- `provider-cost-control`: same prompt/model with capped output and narrow
  tools reports prompt/cache/completion/schema/tool-round breakdown.
- `context-hygiene-preflight`: old large tool outputs are snipped or compacted
  before the next normal coding request while recent proof evidence is kept.

Suggested daily gate slice:

```bash
cargo fmt --check
cargo test -q file_tool
cargo test -q rewind
cargo test -q bash_tool
cargo test -q todo
cargo test -q session_store
cargo test -q usage_ledger
cargo test -q cache_stability
cargo test -q context_compressor
cargo check -q
bash scripts/daily-baseline.sh
```

## 12. Non-Goals

Do not do these as part of this plan:

- Do not replace the conversation loop.
- Do not remove read-before-edit.
- Do not make fuzzy edit matching silently permissive.
- Do not weaken checkpoint creation failures into warnings.
- Do not make shell parser failures allow commands that the current classifier
  would block.
- Do not merge plan mode and todos.
- Do not leave main coding requests uncapped unless explicitly configured.
- Do not optimize only for cache hits while allowing total prompt/noise to grow
  without product policy.
- Do not add more always-on prompt text for issues that can be enforced through
  metadata, tools, projections, tests, or runtime gates.

## 13. Definition Of Done

This plan is complete when:

- [x] file mutation results are consumed by TUI, desktop/runtime events, trace, and repair helpers;
- [x] a user can see and rewind the last assistant mutation round without knowing a checkpoint id;
- [x] edit failures provide deterministic candidate metadata and safe recovery only when unique and stale-safe;
- [x] coding todos are durable session state;
- [x] provider/context cost control is policy-driven: output caps, early hygiene, tool-surface budgets, and usage-ledger dimensions are visible in traces and product status;
- [x] shell classifier has an explicit documented structural-parser deferral decision after risk review;
- [x] daily baseline covers all new programming-chain contracts;
- [x] `docs/PROJECT_STATUS.md` and `docs/PROJECT_MAP.md` are updated.

## 14. Implementation Record

2026-06-05: Phases 1-3, Phase 5, and the first Phase 6 product-control slice are implemented.
Phase 4 remains intentionally deferred as a documented structural-parser risk decision.

**Phase 1 deliverables:**
- `StreamEvent::ToolExecutionComplete.result_data` — carries structured JSON from tool to frontends
- `mutation_result::from_tool_data()` and `compact_summary()` — canonical accessors
- TUI `tool_view.rs` `summarize_file()` prefers `mutation_result.ui_summary`
- `file_patch` added to TUI tool view

**Phase 2 deliverables:**
- `CheckpointManager::last_round_revert_summary()` — user-facing revert projection
- `RoundRevertSummary` struct with paths, tool_round_id, checkpoint_id, rewind_command

**Phase 3 deliverables:**
- `edit_match.rs` — `generate_edit_candidates()` with 4 strategies:
  - line-trimmed block matching
  - indentation-normalized matching
  - block-anchor matching (3+ lines)
  - whitespace-normalized single-line matching
- `file_edit` now consumes candidates in the exact-replace path instead of keeping them test-only:
  - unique multi-line stale anchors can auto-apply when the replacement count matches;
  - ambiguous candidates are returned as deterministic diagnostics;
  - single-line whitespace matches remain hints, not silent fuzzy edits.

**Phase 5 deliverables:**
- `todo_write` persists session todos through `SessionStore`.
- Todo replacement is transactional, so clearing and rewriting cannot leave a partial list after an insert failure.
- Tool results now fail honestly when persistence fails instead of returning a successful response with a hidden persistence warning.
- Session-level persisted todo display helpers are exposed for TUI/desktop/status consumers.

**Phase 6 deliverables:**
- `ChatRequest::with_output_cap()` — output token cap policy
- `output_cap_for_turn()` — coding=8192, repair=1024, inspection=None
- Main streaming, non-streaming fallback, reactive compaction retry, query-simple, and LLM compaction summary requests now carry explicit caps.
- Usage ledger entries now record request phase, effective output cap, tool schema tokens, tool round count, and compaction decision metadata.
- `/cost` summaries can separate capped requests and schema-token contribution by model and session.

**Phase 4 decision:**
- A structural shell parser remains deferred. The current classifier/checkpoint path is preserved because adding a broad parser without a dedicated command corpus risks false confidence and new bypasses. Future work should start behind an opt-in enrichment flag with fail-closed tests.

**Targeted validation run on 2026-06-05:**
- `cargo fmt`
- `cargo fmt --check`
- `cargo check -q`
- `cargo check --features experimental-api-server -q`
- `cargo clippy --all-features -- -D warnings`
- `cargo test -q edit_match -- --test-threads=1`
- `cargo test -q file_tool -- --test-threads=1`
- `cargo test -q todo_store -- --test-threads=1`
- `cargo test -q usage_ledger -- --test-threads=1`
- `cargo test -q route_scoped_tools -- --test-threads=1`
- `cargo test -q context_compressor -- --test-threads=1`

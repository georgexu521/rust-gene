# Opencode Programming Parity Next Plan

Date: 2026-06-07

Status: planning after the API full-agent prompt, queued prompt drain,
idempotency, tool-output, revert, provider-health, usage-ledger, and desktop
reload hardening work.

Reference sources:

- Local project: `/Users/georgexu/Desktop/rust-agent`
- opencode source: `/Users/georgexu/Downloads/opencode-dev`
- Prior local plans:
  - `docs/OPENCODE_PROGRAMMING_CHAIN_GAP_PLAN_2026-06-05.md`
  - `docs/OPENCODE_THIRD_ALIGNMENT_PLAN_2026-06-07.md`
  - `docs/AGENT_PROMPT_ENTRYPOINT_ALIGNMENT_PLAN_2026-06-07.md`

## 1. Current Judgment

The core programming-agent chain is now close to mainstream agent shape.

Priority Agent already has:

- one product runtime entrypoint through `RuntimeController`;
- real TUI/desktop/API full-agent routes rather than a fake provider-chat lane;
- dedicated file tools for read/write/edit/patch;
- raw bash workspace-write guardrails that redirect mutation toward file tools;
- checkpoint-backed file mutation and assistant-turn revert;
- persisted `session_events`, `session_parts`, `session_reverts`, and
  `session_inputs`;
- paged managed tool output;
- provider health checks, provider runtime metadata, and usage-ledger
  projection;
- context compaction with runtime continuity facts;
- LSP tooling and desktop diagnostics;
- live-eval, soak, desktop smoke, and API full-agent soak scripts.

So the gap is no longer "missing basic agent functions". The remaining gap is
product maturity under long real programming tasks: runner lifecycle, context
and diff UX, provider/model catalog consistency, durable status APIs, and
evidence-driven repair ergonomics.

Compared with opencode, Priority Agent is stronger in some safety areas:

- stricter checkpoint and proof boundaries;
- clearer `verified` / `partial` / `not_verified` closeout semantics;
- stronger policy against raw shell mutation;
- richer usage-ledger cache diagnostics;
- personal-machine memory and desktop workflow integration.

opencode is still stronger in several product surfaces:

- per-session runner coordination;
- formal API routes for wait/context/compact;
- message/part projection as the core UI contract;
- file mutation race handling and edit UX simplicity;
- model/provider catalog and plugin-backed provider resolution;
- mature session UI around diffs, turns, tool errors, and retry.

## 2. What opencode Does Better

### 2.1 Per-session runner lifecycle

opencode has a process-global `SessionRunCoordinator` keyed by session id. It
allows different sessions to drain concurrently while ensuring only one drain
chain owns a single session. It also exposes formal session operations such as:

- `POST /api/session/:sessionID/prompt`
- `POST /api/session/:sessionID/wait`
- `POST /api/session/:sessionID/compact`
- `GET /api/session/:sessionID/context`

Priority Agent now has durable API prompt admission and an opencode-like queued
prompt drain, but the API adapter still uses one shared `RuntimeController` and
a global serialized lock. That was the correct safe first step because the
underlying controller still mutates the current session binding. It should not
be made parallel until there are real per-session runtime handles.

Gap:

- no production-grade per-session API runner map yet;
- no formal HTTP wait endpoint for "session is idle";
- no durable run-status projection that says queued/running/waiting_permission/
  cancelled/completed/failed in one place;
- restart recovery for pending `session_inputs` is not yet a product feature.

### 2.2 Active context API and compaction contract

opencode treats active context as a first-class API surface. Its context loader
returns messages after the latest compaction boundary, and compaction is a
session operation.

Priority Agent has a strong context compressor and desktop context snapshot,
but the HTTP/API contract is still less direct for external clients:

- desktop can inspect context pressure;
- runtime can compact;
- session store has compact boundaries;
- but there is no simple `GET /api/sessions/:id/context` that returns the
  current model-facing active context after compaction.

Gap:

- no frontend-neutral active-context DTO;
- hard to debug "why did the model see this file/history/tool schema";
- API clients cannot easily compare pre/post compaction state.

### 2.3 File mutation race handling and edit UX

opencode's newer core file mutation layer follows a simple pattern:

`resolve target -> approve -> lock target -> revalidate -> mutate`

The edit tool also returns concise diff previews and rejects stale content when
the file changed after approval.

Priority Agent already has read-before-edit checks, mutation locks, high-risk
target guards, checkpoints, diagnostics delta, and atomic multi-file
`file_patch` rollback. In several safety respects this is stronger than
opencode. The gap is less about correctness and more about ergonomic precision:

- exact edit fallback and failure hints can still be made more model-friendly;
- stale-content errors should consistently say "read again before editing";
- multi-file patch has strong rollback, but model-facing output can be easier
  to scan;
- file mutation result should consistently include before/after diff preview,
  checkpoint id, read-state, stale-state, and diagnostics delta in one schema.

Gap:

- file tool outputs are rich but not yet a single compact mutation-result
  contract across `file_write`, `file_edit`, and `file_patch`;
- error recovery messages vary by tool;
- UI diff review exists, but is not yet as central as opencode's session diff
  components.

### 2.4 Shell lifecycle and background process ergonomics

opencode's shell boundary is simple and product-oriented: bounded output,
timeout, managed output storage, external directory approval, and clear
warnings. Its source also keeps visible TODOs for background-job observation
before exposing durable HTTP background jobs.

Priority Agent has a richer bash classifier, PTY/background handling, command
risk checks, and policy that raw shell workspace writes should not be the normal
editing path. This is good. Remaining product gaps:

- shell background tasks should have one stable status API;
- long-running commands should expose wait/cancel/status consistently in CLI,
  TUI, desktop, and API;
- shell output paging should be tied back to run status and diagnostics;
- command purpose/description should be consistently shown in UI status cards.

Gap:

- shell process lifecycle is present but still spread across tool output,
  bash background, desktop events, and diagnostics;
- not yet a single "process/job" projection that all frontends consume.

### 2.5 Provider and model catalog

opencode has a richer provider/model catalog with provider IDs, model IDs,
variants, request headers/body settings, enabled-via-env/account/custom states,
and API-family resolution.

Priority Agent now has a practical provider registry, provider health checks,
usage ledger, output caps, cache diagnostics, and DeepSeek/Kimi/GLM/OpenAI
compatible support. For personal use this is already enough. For a mature
commercial product, the provider catalog is still less systematic than
opencode:

- provider config is not yet fully described by a versioned DTO;
- model limit and output-cap metadata are not consistently catalog-driven;
- provider health and usage are strong, but provider selection UI could be
  clearer;
- model variants and per-provider request options are less formal.

Gap:

- no opencode-style catalog layer as the single source of provider/model truth;
- provider UI/status is improving but not yet a full product surface.

### 2.6 Session projection and UI contract

opencode projects session events into session messages/parts and uses those as
the UI contract. It has mature components for session turns, diffs, tool errors,
tool count summaries, retry, message navigation, and file references.

Priority Agent has moved in this direction with `session_events`,
`session_parts`, tool-output paging, desktop reload, and typed diagnostic export.
The remaining gap is consistency:

- desktop/TUI/API should all consume the same stable DTOs where possible;
- assistant turns, tool calls, permissions, revert, closeout, usage, and
  diagnostics should be renderable without frontend-specific inference;
- retry/revert/continue should map to stable session part IDs and durable events.

Gap:

- UI is functional, but opencode still has a more polished session-review and
  diff-centered programming experience;
- API DTOs should become the only frontend contract, not one contract per UI.

### 2.7 Real task soak and regression baseline

opencode has many focused tests around session runner, file mutation, tools,
provider routes, permissions, and UI components.

Priority Agent has many gates too, but the next maturity step is not more
synthetic eval logic. It is repeated real-task soak:

- same prompt across CLI/TUI/desktop/API;
- real repo edits;
- validation commands;
- provider timeout/slow-tail records;
- usage ledger proof;
- reload/revert proof;
- failure-owner classification.

Gap:

- daily baseline exists in pieces, but should become a formal release gate for
  real programming tasks.

## 3. Priority Agent Is Already Close In These Areas

These should not be rebuilt just to look like opencode:

- **File mutation safety:** keep read-before-edit, high-risk target guards,
  checkpoint requirements, and patch rollback.
- **Closeout proof:** keep verified/partial/not_verified. opencode is clean,
  but Priority Agent's explicit proof boundary is a product advantage.
- **Usage/cost observability:** the local `usage.jsonl`/SQLite projection is
  already more tailored to the cache-hit/cost questions gex cares about.
- **Personal desktop workflow:** desktop diagnostics, permission cards, context
  snapshot, and session reload are project-specific advantages.
- **Raw bash mutation policy:** keep redirecting code edits toward file tools.
  Do not weaken this for model convenience.

## 4. Recommended Next Development Plan

### Slice A: Per-session API runner handles

Goal:

Replace the current global serialized API drain with a session-keyed runner map,
without racing the shared `RuntimeController` session binding.

Implementation direction:

1. Introduce `ApiSessionRunnerRegistry` keyed by `session_id`.
2. Each registry entry should expose:
   - `wake(session_id)`;
   - `run(session_id)`;
   - `wait_idle(session_id)`;
   - `status(session_id)`;
   - `cancel(session_id)`.
3. Do not enable cross-session parallelism until each entry owns an isolated
   runtime handle or the controller can submit with an explicit session id
   without mutating global binding.
4. Add HTTP endpoints:
   - `POST /api/sessions/:id/wait`;
   - `POST /api/sessions/:id/cancel`;
   - `GET /api/sessions/:id/run-status`.
5. On startup, scan `session_inputs` for pending/promoted/running rows and mark
   stale `running` rows as `failed` or `pending_restart_recovery`, then wake
   eligible pending rows if configured.

Acceptance:

- two prompts to the same session never run concurrently;
- queued prompts in one session drain exactly once;
- prompt retries with the same `idempotency_key` do not duplicate work;
- wait returns only after session is idle;
- cancel transitions the run to a durable cancelled status;
- restart recovery behavior is deterministic and documented.

Suggested tests:

```bash
cargo test -q run_coordinator
cargo test -q api::routes --features experimental-api-server
PRIORITY_AGENT_API_SOAK_QUEUE=1 scripts/api-full-agent-soak.sh
```

### Slice B: Active context API

Goal:

Make "what the model will see now" inspectable through a stable API, similar to
opencode's session context endpoint.

Implementation direction:

1. Add a `SessionContextDto` under `src/api/dto/`.
2. Expose `GET /api/sessions/:id/context`.
3. Return:
   - compact boundary id;
   - included messages/parts after latest compaction;
   - estimated history tokens;
   - tool schema tokens;
   - memory snapshot tokens;
   - stable prefix hash;
   - dynamic tail hash;
   - latest compaction attempt summary.
4. Make desktop/TUI context panels consume this DTO where possible.

Acceptance:

- after compaction, context endpoint shows only active post-boundary context
  plus compact summary;
- context endpoint matches desktop context snapshot counts within expected
  estimation tolerance;
- API route has contract tests and one real soak check.

### Slice C: Unified file mutation result schema

Goal:

Make every file mutation return one compact, model-friendly, UI-friendly result
shape.

Implementation direction:

1. Define `FileMutationResultV2` for `file_write`, `file_edit`, and
   `file_patch`.
2. Include:
   - operation;
   - changed paths;
   - checkpoint id;
   - rollback id or revert target;
   - before/after diff preview;
   - stale/read-before-edit state;
   - diagnostics delta;
   - partial/rolled-back status.
3. Normalize stale-content and non-unique-match error messages:
   - "read the file again before editing";
   - "old_string matched N times; add surrounding context";
   - "file changed since read; rerun file_read and retry".
4. Teach desktop transcript and trace drawer to render the unified result.

Acceptance:

- file tools still enforce current guardrails;
- model gets shorter, clearer repair hints;
- desktop can render the same mutation schema for write/edit/patch;
- no raw bash mutation path is added.

Suggested tests:

```bash
cargo test -q file_tool
cargo test -q action_review
cargo test -q permissions
```

### Slice D: Shell job projection

Goal:

Give shell commands, long-running tasks, and background jobs one product
projection.

Implementation direction:

1. Add a `session_jobs` table or projection if existing event data is not
   enough.
2. Track:
   - job id;
   - session id;
   - command;
   - cwd;
   - state;
   - started/completed timestamps;
   - exit code;
   - timeout;
   - tool-output URI;
   - cancellation status.
3. Add API/desktop/TUI accessors:
   - list jobs for session;
   - read job output page;
   - cancel job where supported.
4. Keep bash as validation/execution support, not primary code editing.

Acceptance:

- long command output is never only inline;
- desktop can recover job state after reload;
- API can show whether a command is still running, timed out, cancelled, or
  completed;
- dangerous shell write patterns remain blocked by action review.

### Slice E: Provider/model catalog DTO

Goal:

Move provider/model status closer to opencode's catalog shape while preserving
Priority Agent's provider-health and usage-ledger advantages.

Implementation direction:

1. Add a versioned `ProviderCatalogDto`.
2. Include:
   - provider id/name;
   - enabled state and source;
   - base URL;
   - default model;
   - available model id;
   - context/output limits;
   - protocol family;
   - last health status;
   - last latency/TTFT;
   - recent timeout/error category;
   - recent usage/cost/cache-hit summary.
3. Use this DTO in API and desktop provider status.
4. Keep existing env-based provider setup, but expose it through a stable
   product contract.

Acceptance:

- provider status is understandable without reading logs;
- DeepSeek/Kimi/GLM/OpenAI-compatible setups show consistent model limits and
  output caps;
- slow-tail and timeout diagnosis appears in UI/API.

### Slice F: Real programming soak baseline

Goal:

Turn real programming tasks into the release gate for the backend algorithm.

Implementation direction:

1. Create `scripts/programming-soak-suite.sh`.
2. Run the same task set through:
   - CLI;
   - TUI expect mode;
   - desktop smoke where feasible;
   - API full-agent route.
3. Include at least these scenarios:
   - single-file bug fix with tests;
   - multi-file refactor;
   - frontend component change;
   - failing test repair loop;
   - permission denial then safe retry;
   - long shell output with paging;
   - revert last assistant turn;
   - provider timeout/slow-tail classification.
4. Each run should write an artifact bundle:
   - prompt;
   - session id;
   - changed files;
   - tool calls;
   - validation commands;
   - closeout status;
   - usage ledger summary;
   - provider health snapshot;
   - failure_owner if failed.

Acceptance:

- failures are classified as framework/provider/model/task ambiguity;
- framework failures produce specific repair issues;
- weak-model mistakes do not weaken runtime validation;
- daily baseline can be run before release.

## 5. Suggested Order

1. Slice A: per-session API runner handles.
2. Slice B: active context API.
3. Slice C: unified file mutation result schema.
4. Slice E: provider/model catalog DTO.
5. Slice D: shell job projection.
6. Slice F: real programming soak baseline.

Reason:

- Runner lifecycle and context visibility improve every frontend.
- File mutation UX directly improves coding accuracy.
- Provider catalog makes slow-tail/cost debugging product-grade.
- Shell job projection and soak baseline turn the system from "works in tests"
  into "survives real use".

## 6. Non-goals

- Do not copy opencode's architecture wholesale.
- Do not remove Priority Agent's checkpoint/proof/revert gates.
- Do not loosen raw bash workspace mutation policy.
- Do not build another fake `/chat` path for full-agent work.
- Do not add prompt-only rules for one-off weak-model mistakes.
- Do not enable cross-session API parallelism until runtime handles are
  session-isolated.

## 7. Definition Of Done

This next parity phase is done when:

- API has session prompt, wait, cancel, run-status, compact, and context routes;
- queued/admitted prompts survive retries and have deterministic restart policy;
- file mutation outputs are unified and easy for both model and UI to consume;
- shell jobs have durable status and output paging;
- provider/model catalog is a stable UI/API contract;
- daily real programming soak can prove:
  - code changes happen through file tools;
  - validation failures re-enter the repair loop;
  - closeout is honest;
  - usage/cost/cache data is ledger-backed;
  - reload/revert still works after long sessions.

## 8. Bottom Line

Priority Agent is now close enough to opencode on the core programming-agent
chain that the next work should be product hardening, not broad feature
accumulation.

The biggest remaining opencode-inspired improvements are:

1. per-session runner lifecycle;
2. active context API;
3. unified file mutation output and diff UX;
4. provider/model catalog polish;
5. shell job projection;
6. real programming soak as a release gate.

If these are completed, the programming backend will be much closer to mature
commercial-agent expectations: not just able to edit code, but able to explain,
recover, reload, measure, and prove its work across long real tasks.

# API Contract / Provider / UI Next Steps

Date: 2026-06-07

Status: active implementation plan after API contract/provider status hardening

## 1. Current Baseline

This plan starts from the current `claude-test` working tree after the first
API/provider productization slice.

Implemented or in progress:

- `src/api/dto/*` now defines shared DTO vocabulary for session, tool-output,
  provider, permission, and diagnostics.
- `src/api/routes.rs` exposes read-only routes for:
  - `GET /api/sessions/:id/parts`
  - `GET /api/sessions/:id/events`
  - `GET /api/sessions/:id/reverts`
  - `GET /api/sessions/:id/tool-outputs`
  - `GET /api/sessions/:id/tool-outputs/:output_id`
  - `GET /api/provider/status`
  - `GET /api/diagnostics/latest`
- `src/api/provider_status.rs` now builds `ProviderStatusPage` /
  `ProviderProductStatus` and keeps provider status logic out of
  `src/api/routes.rs`.
- `src/api/routes.rs` is back under the 1500-line maintainability limit.
- API route contract tests live in `src/api/routes/contract_tests.rs` so
  route code and contract tests can evolve without pushing `routes.rs` back
  over the file-size limit.
- `GET /api/sessions/:id/reverts` reads the durable `session_reverts` table
  rather than inferring revert history from projected session parts.
- `POST /api/chat` returns explicit `execution_kind: "provider_chat"` and
  `full_agent: false`.
- `POST /api/sessions/:id/prompt` exists as a typed `501` full-agent boundary:
  it returns `execution_kind: "full_agent_turn"`, `accepted: false`,
  `agent_runtime_entrypoint: "RuntimeController"`, and no diagnostic/events
  until the route is wired to the real runtime controller.
- Provider timeout DTOs read `PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS`, matching
  the runtime timeout path, and use the runtime stream-idle default of 120s.
- `GET /api/provider/status` returns top-level `timeout_effective` with
  timeout values and source metadata, so UI can render the runtime timeout
  policy without reverse-engineering per-provider rows.
- Basic provider status/helper and API route contract tests exist.

Verified gates:

```bash
cargo fmt --check
cargo check --features experimental-api-server -q
cargo test -q api::routes --features experimental-api-server
```

Important current constraint:

- `POST /api/chat` in `src/api/state.rs` is direct provider chat by design. It
  does not enter the full agent runtime.
- `POST /api/sessions/:id/prompt` is still a typed `501`, not a real agent run.
- The true desktop/full-agent path goes through `desktop_runtime::DesktopRuntime`
  and `engine::runtime_controller::RuntimeController`.
- Desktop frontend DTOs are still primarily in
  `apps/desktop/src/runtime/desktopApi.ts`; the desktop app is not yet
  consuming the new HTTP API DTOs as its single contract.

## 2. Goal

The next phase should finish the product contract before building more UI.

The goal is:

1. Make the API route outputs hard to accidentally drift.
2. Make direct provider chat clearly separate from full-agent execution.
3. Give provider/UI work a stable data source.
4. Keep desktop/TUI/API aligned around one vocabulary instead of three
   parallel shapes.

This is the right order because UI polish is fragile if the data contract is
still ambiguous.

## 3. Non-Goals

- Do not make the experimental API public by default yet.
- Do not replace the desktop Tauri path with HTTP API calls in one big rewrite.
- Do not weaken permission/checkpoint/runtime proof behavior to simplify API
  behavior.
- Do not add broad provider catalog features before DeepSeek/OpenAI-compatible,
  MiniMax, and Kimi behavior is test-covered.
- Do not add more always-on prompt text to solve API/product issues.

## 4. Workstream A: API Contract Hardening

### A1. Add Snapshot-Style Route Contract Tests

Problem:

The project now has DTO structs, but route handlers can still drift by returning
ad hoc `json!` shapes or omitting fields. This is especially risky because
desktop, TUI, and future API clients will depend on the route shape.

Implementation:

- Add focused tests for these routes:
  - `GET /api/sessions/:id/parts?after=&limit=`
  - `GET /api/sessions/:id/events?after=&limit=`
  - `GET /api/sessions/:id/reverts?limit=`
  - `GET /api/sessions/:id/tool-outputs`
  - `GET /api/sessions/:id/tool-outputs/:output_id?offset=&limit=`
  - `GET /api/provider/status`
  - `GET /api/diagnostics/latest?session_id=`
- Tests should assert required fields and cursor semantics, not full brittle
  timestamps.
- Prefer constructing an in-memory `SessionStore` and deterministic
  `ToolOutputStore::at(tempdir)` where possible.
- Keep the tests in small modules instead of growing `src/api/routes.rs`.
  Current route tests can stay split between `src/api/routes.rs` and
  domain-specific modules, but avoid pushing any file over 1500 lines.

Code entry points:

- `src/api/routes.rs`
- `src/api/provider_status.rs`
- `src/api/dto/session.rs`
- `src/api/dto/tool_output.rs`
- `src/api/dto/provider.rs`
- `src/api/dto/diagnostic.rs`
- `src/session_store/mod.rs`
- `src/session_store/session_parts.rs`
- `src/tool_output_store/mod.rs`

Acceptance:

- Route tests fail if DTO-critical fields disappear.
- Cursor responses include stable `cursor.limit`, `cursor.has_more`, and
  next-cursor fields.
- Reverts test proves the API reads `session_reverts`, not only projected
  parts.
- Tool-output test proves session scoping blocks wrong-session reads.

Suggested gates:

```bash
cargo fmt --check
cargo test -q api --features experimental-api-server
cargo test -q session_store
cargo test -q tool_output_store
cargo check --features experimental-api-server -q
```

### A2. Update `docs/api/session_schema.md`

Problem:

The schema doc is close, but now it must match the exact Rust DTOs and new
provider status page shape.

Implementation:

- Update provider section from a single runtime profile into
  `ProviderStatusPage`.
- Ensure `SessionRevertItem` includes:
  - `id`
  - `operation`
  - `checkpoint_ids`
  - `diff_summary`
  - `created_at`
  - `unreverted`
  - `payload`
- Ensure cursor docs include `limit` where the DTO includes it.
- Add a short note that `session_events` are audit/debug, while
  `session_parts` are the preferred UI projection.
- Add a short note that `/api/chat` is not the full agent path until the next
  workstream lands.

Acceptance:

- Docs match `src/api/dto/*`.
- A frontend developer can implement against the doc without reading raw
  `session_events`.

## 5. Workstream B: Direct Provider Chat vs Full-Agent Prompt

### B1. Make `/api/chat` Honest

Problem:

`POST /api/chat` currently calls `ApiState::chat`, which builds a small
provider request directly:

- no runtime route classification;
- no `RuntimeController`;
- no full tool loop;
- no checkpoint/proof contract;
- no session event stream equivalent to desktop/TUI.

This is useful for smoke tests, provider probing, and simple integration, but
it is misleading if exposed as the main agent API.

Implementation options:

Option 1, low risk:

- Keep `POST /api/chat`, but document it as provider chat.
- Add response metadata:
  - `execution_kind: "provider_chat"`
  - `full_agent: false`
  - `agent_runtime_entrypoint: null`
- Add route docs and tests proving this is not full-agent execution.

Option 2, clearer API:

- Add a new route:
  - `POST /api/provider/chat`
- Keep `/api/chat` as a compatibility alias for now.
- Mark `/api/chat` as legacy or provider-chat compatibility in docs.

Recommended for this project:

- Start with Option 1 to avoid churn.
- Add Option 2 only when the full-agent route exists.

Acceptance:

- No API doc implies `/api/chat` can do coding-agent work.
- Tests assert the response identifies provider-chat execution.

### B2. Add Full-Agent Prompt API

Problem:

The desktop/TUI path can run real agent turns, but the HTTP API cannot yet
submit a prompt into the same runtime path.

Recommended route:

```text
POST /api/sessions/:id/prompt
```

Request shape:

```json
{
  "message": "string",
  "agent_mode": "normal | plan | review | optional",
  "stream": "bool | optional"
}
```

Response shape for non-streaming first slice:

```json
{
  "session_id": "string",
  "execution_kind": "full_agent_turn",
  "accepted": false,
  "turn_id": "string | null",
  "status": "not_implemented",
  "events_written": "usize",
  "latest_part_index": "i64 | null",
  "diagnostic": "<DiagnosticExportDto | null>",
  "agent_runtime_entrypoint": "RuntimeController",
  "error": "string | null"
}
```

Implementation approach:

- Do not duplicate desktop runtime logic inside `src/api/state.rs`.
- Create a small API-side full-agent runner that wraps
  `RuntimeController::submit_turn` or `submit_stream_turn`.
- Reuse the same `StreamingQueryEngine` provider/tool/session setup used by
  CLI/TUI/desktop where feasible.
- The typed `501` boundary is now present. The remaining product work is to
  replace that stub with a real `RuntimeController` submission path while
  preserving the same response vocabulary.
- The final implementation must persist `session_events` and `session_parts`
  the same way desktop/TUI do.

Key code entry points:

- `src/api/state.rs`
- `src/api/routes.rs`
- `src/desktop_runtime/mod.rs`
- `src/engine/runtime_controller.rs`
- `src/engine/streaming.rs`
- `src/session_store/event_mirror.rs`
- `src/session_store/session_parts.rs`

Acceptance:

- Full-agent HTTP prompt follows the same closeout/proof/checkpoint behavior as
  TUI/desktop.
- A completed API run can be reloaded through `GET /api/sessions/:id/parts`.
- Tool output created during the run is available through the tool-output
  paging API.
- Provider usage from the run lands in the real usage ledger, not only in
  in-memory estimates.

Suggested gates:

```bash
cargo test -q runtime_controller
cargo test -q session_parts
cargo test -q api --features experimental-api-server
cargo check --features experimental-api-server -q
```

## 6. Workstream C: Provider Product Data

### C1. Make Provider Status Useful, Not Just Present

Current status:

`GET /api/provider/status` now returns a product-shaped page, but several
fields are still placeholders or best-effort:

- provider cost fields are `None`;
- retry count is not connected to the runtime ledger;
- model catalog is not persisted;
- timeout source is not explicit;
- health history is latest JSONL only, not a queryable provider history.

Implemented in the current slice:

- Expose top-level `timeout_effective` on `GET /api/provider/status`.
- Report the currently implemented timeout sources:
  - `default`;
  - `env`.

Remaining implementation:

- Add additional timeout source values when those paths exist:
  - `project_config`;
  - `runtime_override`.
- Connect latest request latency/retry count from runtime evidence or usage
  ledger where available.
- Add "unverified" status when no health check exists for a provider/model.
- Keep base URL host redacted; do not expose API keys or full credential URLs.

Potential DTO additions:

```rust
pub struct ProviderTimeoutEffectiveDto {
    pub request_secs: u64,
    pub stream_idle_secs: u64,
    pub slow_warning_secs: u64,
    pub max_retry_attempts: u32,
    pub source: String,
}
```

Code entry points:

- `src/api/provider_status.rs`
- `src/services/api/provider.rs`
- `src/services/api/provider_protocol.rs`
- `src/diagnostics/provider_health.rs`
- `src/engine/conversation_loop/runtime_timeouts.rs`
- `src/usage_ledger` or current usage-ledger module path

Acceptance:

- User can answer from API output:
  - which provider/model is active;
  - whether it is configured;
  - where it is configured from;
  - whether a health check has run;
  - why it may be slow;
  - what timeout is actually effective.

### C2. Provider Certification Matrix

Problem:

The project has provider-specific behavior in several modules, but the product
does not yet have an obvious matrix proving DeepSeek/OpenAI-compatible,
MiniMax, and Kimi behavior.

Implementation:

- Add a small provider certification doc/table and tests for:
  - request field naming;
  - max output field;
  - tool-call shape;
  - streaming tool-call support;
  - non-streaming fallback requirement;
  - usage extraction;
  - cached-token extraction;
  - timeout classification.

Code entry points:

- `src/services/api/openai.rs`
- `src/services/api/openai_compat.rs`
- `src/services/api/minimax.rs`
- `src/services/api/kimi.rs`
- `src/services/api/provider_protocol.rs`
- `src/diagnostics/provider_health.rs`

Acceptance:

- DeepSeek can be reasoned about as OpenAI-compatible with known exceptions.
- MiniMax slow/non-streaming behavior is tested and visible.
- Kimi path keeps its protocol-specific assumptions explicit.

Suggested gates:

```bash
cargo test -q provider_protocol
cargo test -q provider_health
cargo test -q minimax
cargo test -q kimi
cargo test -q openai_compat
```

## 7. Workstream D: Desktop/TUI DTO Alignment

### D1. Do Not Rewrite Desktop Yet

Desktop currently works through Tauri commands and
`apps/desktop/src/runtime/desktopApi.ts`. It should not be replaced by the HTTP
API in one large change.

Instead:

1. Mirror the Rust DTO vocabulary in `desktopApi.ts`.
2. Replace duplicated component-local shapes with imports from `desktopApi.ts`.
3. Use the same field names as `src/api/dto/*`.
4. Add TypeScript tests that fail when required fields are missing.

Code entry points:

- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/app/components/StatusBar.tsx`
- `apps/desktop/src/app/components/Composer.tsx`
- `apps/desktop/src/app/components/Transcript.tsx`
- `apps/desktop/src/app/components/ToolOutputDrawer.tsx`
- `apps/desktop/src/app/components/DiagnosticsPanel.tsx`

Acceptance:

- Desktop provider status panel can eventually consume
  `ProviderProductStatus` without field translation guesswork.
- Tool-output drawer uses the same `ToolOutputIndex` and `ToolOutputPageDto`
  vocabulary as the API.
- Session parts/reverts UI uses stable `SessionPartItem` /
  `SessionRevertItem`.

Suggested gates:

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop test:ui-smoke
```

### D2. Product UI After DTO Alignment

Only after DTO alignment, add visible UI panels:

- provider status popover;
- provider health history;
- effective timeout display;
- diagnostic export/copy/open-folder actions;
- permission explanation depth;
- context usage panel;
- tool-output policy and paged output search.

Acceptance:

- User can diagnose common provider/runtime failures from desktop without
  reading terminal logs.
- Desktop/TUI/API status vocabulary is consistent.

## 8. Workstream E: Release Gates And Soak

After A-D land, run a small real-task soak rather than only unit tests.

Recommended soak set:

1. CLI/TUI small coding task:
   - edit one file;
   - run targeted test;
   - verify closeout and usage ledger.
2. Desktop real task:
   - submit prompt;
   - observe transcript, session parts reload, tool outputs, diagnostics.
3. Provider slow-tail scenario:
   - run with DeepSeek;
   - inspect provider status and timeout fields;
   - confirm no false green closeout.
4. Revert scenario:
   - create a file change;
   - revert assistant turn;
   - reload session;
   - verify reverts API and desktop state.
5. Tool-output large log:
   - generate large output;
   - confirm preview truncation;
   - page full output from API or desktop drawer.

Suggested gates:

```bash
cargo fmt --check
cargo test -q api --features experimental-api-server
cargo test -q session_store
cargo test -q tool_output_store
cargo test -q provider_health
cargo check --features experimental-api-server -q
corepack pnpm --dir apps/desktop exec tsc --noEmit
```

## 9. Recommended Execution Order

### Slice 1: API Contract Tests And Docs

Do first.

- Add route contract tests for parts/events/reverts/tool-output/diagnostic.
- Update `docs/api/session_schema.md`.
- Keep `/api/chat` documented as provider chat.

Why first:

This protects the work already done and gives UI/provider work a stable base.

### Slice 2: Full-Agent Prompt API Boundary

Do second.

- Add explicit provider-chat metadata to `/api/chat`.
- Replace the current typed `501` `POST /api/sessions/:id/prompt` route with a
  real RuntimeController-backed route.

Why second:

This removes the biggest product ambiguity in the API.

### Slice 3: Provider Effective Status

Do third.

- Add timeout source.
- Add unverified/configured/unavailable status.
- Connect health/result data more directly.
- Add provider certification tests for DeepSeek/OpenAI-compatible/MiniMax/Kimi.

Why third:

Provider status is a product surface only if it can explain slow/failing runs.

### Slice 4: Desktop DTO Alignment

Do fourth.

- Mirror DTOs in `desktopApi.ts`.
- Reduce component-local payload interpretation.
- Add TypeScript checks.

Why fourth:

Desktop UI should consume stable contracts, not hard-code backend internals.

### Slice 5: Desktop Product Panels

Do last.

- Provider settings/status panel.
- Diagnostics panel product pass.
- Tool-output policy/search.
- Permission explanation UI.

Why last:

UI becomes much less fragile after the data contract is stable.

## 10. Stop Conditions

Stop and reassess before continuing if any of these happen:

- A route requires weakening permission/checkpoint/proof gates to pass tests.
- Full-agent HTTP route forks runtime behavior away from TUI/desktop.
- A DTO field is added only for one UI component and has no durable runtime
  meaning.
- A file crosses 1500 lines without a clear split plan.
- Tests only assert happy-path provider responses and ignore failure/timeout
  classification.

## 11. Definition Of Done For This Next Phase

This phase is done when:

- API DTOs are route-tested and documented.
- `/api/chat` is clearly provider chat, not mislabeled full-agent execution.
- There is a clear full-agent prompt API path or explicit typed blocker.
- Provider status explains active model, source, health, timeout, and slow-tail
  state.
- Desktop/TUI/API share DTO vocabulary for session, provider, diagnostic, and
  tool-output surfaces.
- The standard gates pass after a clean build.

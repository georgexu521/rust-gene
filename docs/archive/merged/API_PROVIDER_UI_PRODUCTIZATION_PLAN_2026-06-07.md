# API / Provider / UI Productization Plan

Date: 2026-06-07

Status: in progress; Phase 1 base API DTO/routes are partially implemented

## 1. Purpose

Priority Agent now has the core programming-agent primitives needed for real
local work: shared runtime entrypoints, durable sessions, `session_events`,
`session_parts`, usage ledger, paged tool output, provider diagnostics,
permissions, checkpoints, revert, and desktop/TUI reload.

This next phase is not about adding broad new agent features. It is about
turning existing internals into product surfaces that are stable, visible,
configurable, and testable. The goal is to make Priority Agent feel less like a
powerful internal tool and more like mature software that can survive long
sessions, slow providers, desktop reloads, and future API clients.

## 2. Sources Reviewed

opencode source:

- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/server/server.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/server/*`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/*`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/config/tool-output.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/config/provider.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/permission/saved.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/provider/model-status.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/app/src/components/status-popover.tsx`
- `/Users/georgexu/Downloads/opencode-dev/packages/app/src/components/settings-v2/providers.tsx`
- `/Users/georgexu/Downloads/opencode-dev/packages/app/src/components/session/*`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/test/server/*`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/test/provider/*`
- `/Users/georgexu/Downloads/opencode-dev/packages/llm/test/provider/*`

Priority Agent source:

- `src/api/dto/*`
- `src/api/provider_status.rs`
- `src/api/routes.rs`
- `src/api/state.rs`
- `src/api/websocket.rs`
- `src/session_store/*`
- `src/services/api/provider.rs`
- `src/services/api/provider_protocol.rs`
- `src/services/api/retry.rs`
- `src/diagnostics/provider_health.rs`
- `src/services/config.rs`
- `src/tool_output_store/mod.rs`
- `src/permissions/mod.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `apps/desktop/src/app/components/*`
- `apps/desktop/src/runtime/desktopApi.ts`
- `docs/api/session_schema.md`

## 3. High-Level Assessment

Priority Agent is no longer missing the core agent runtime pieces. The main
gap versus opencode is product shape:

- opencode has a clearer public server/API layer with OpenAPI-style contracts,
  route tests, auth tests, SDK-facing behavior, and WebSocket lifecycle
  handling.
- opencode treats provider/model/config data as product data, not only runtime
  internals. Provider connection source, model status, model limits, cost, and
  capability metadata flow into settings and status UI.
- opencode UI has richer settings/status surfaces: connected providers, custom
  providers, server status, MCP status, context usage, model selection, and
  session panels are visible and test-covered.

Priority Agent has many of these raw facts now, but they are still split across
TUI commands, desktop status bars, diagnostic JSON, env vars, and internal
Rust structs. The next phase should unify them into stable DTOs and user-facing
views.

## 4. Non-Goals

- Do not build a cloud product or multi-tenant remote service in this phase.
- Do not add many new providers just to match opencode's catalog.
- Do not weaken checkpoints, permission gates, or proof gates for UX polish.
- Do not make the experimental API public by default until contract tests are
  strong.
- Do not rewrite the desktop UI wholesale. Add focused panels and contracts.

## 5. Phase 1: API Contract Productization

### opencode reference

opencode's server path is built around typed route contracts, public API
generation, WebSocket lifecycle management, and extensive route tests. Its
session v2 spec separates durable prompt admission, session events, projected
messages, execution resume, and event cursor replay.

### Priority Agent status

Priority Agent has `experimental-api-server`, REST routes, WebSocket support,
bridge v1 routes, `docs/api/session_schema.md`, shared API DTO modules, and
read-only product routes for session parts, session events, session reverts,
tool-output pages, provider status, and diagnostic snapshots. Remaining product
gaps:

- `POST /api/chat` calls the provider directly rather than the full
  programming-agent runtime path.
- `POST /api/chat/stream` is still not implemented.
- session API returns older message-style data more than typed
  `session_parts`/`session_events` cursors.
- API DTOs now exist in `src/api/dto/*`, but still need stronger schema drift
  snapshot tests and desktop/Tauri adoption.
- desktop uses Tauri command DTOs; experimental HTTP API uses separate route
  structs.

### Implementation Progress 2026-06-07

- Added shared DTO vocabulary under `src/api/dto/*`.
- Added read-only routes for projected session parts, raw session events,
  durable session reverts, session tool-output index/page, provider status, and
  diagnostic snapshots.
- Updated route responses so parts/events/reverts/tool-output/diagnostic use
  concrete DTO structs instead of ad hoc response maps.
- Updated session revert API to read the durable `session_reverts` table rather
  than inferring revert history from projected parts only.
- Expanded provider status into `ProviderStatusPage` /
  `ProviderProductStatus`, including configured/active state, connection
  source, base URL host, protocol family, context/output limits, effective
  timeout fields, capability summary, and matching latest health-ledger result.
- Split provider status DTO assembly into `src/api/provider_status.rs` so
  `src/api/routes.rs` stays under the 1500-line maintainability limit.
- Added focused API route/helper tests for provider status DTO shape, provider
  host parsing, and health-ledger mapping.

### Work Items

1. Create a shared DTO module:
   - `src/api/dto/session.rs`
   - `src/api/dto/tool_output.rs`
   - `src/api/dto/provider.rs`
   - `src/api/dto/permission.rs`
   - `src/api/dto/diagnostic.rs`

2. Move stable structs out of ad hoc route modules:
   - session info;
   - session parts cursor;
   - session events cursor;
   - session reverts;
   - tool-output index/page;
   - provider status/profile;
   - permission explain result;
   - diagnostic export summary.

3. Add API routes around existing durable stores:
   - `GET /api/sessions/:id/parts?after=&limit=`
   - `GET /api/sessions/:id/events?after=&limit=`
   - `GET /api/sessions/:id/reverts?limit=`
   - `GET /api/sessions/:id/tool-outputs`
   - `GET /api/sessions/:id/tool-outputs/:id?offset=&limit=`
   - `GET /api/provider/status`
   - `GET /api/diagnostics/latest?session_id=`

4. Keep full-agent execution separate from raw provider chat:
   - either rename current `POST /api/chat` to explicit provider chat;
   - or add `POST /api/sessions/:id/prompt` that enters the same
     `RuntimeController` path as TUI/desktop;
   - do not pretend direct provider chat is the real programming-agent path.

5. Implement streaming API only after the contract is clear:
   - `GET /api/sessions/:id/events/stream` or WebSocket event stream;
   - replay persisted events first, then live tail;
   - document which deltas are durable and which are ephemeral.

6. Add route contract tests:
   - health/version;
   - session list/detail;
   - session parts cursor;
   - session events cursor;
   - tool-output paging;
   - provider status;
   - auth failures;
   - schema drift snapshots.

### Acceptance

- Desktop, TUI diagnostics, and experimental API use the same DTO vocabulary.
- No frontend component has to reinterpret raw `session_events` payloads.
- API docs match tested route output.
- Direct provider chat is not confused with full agent execution.
- API remains feature-gated until route tests and auth tests pass.

### Suggested Gates

```bash
cargo test -q api
cargo test -q session_store
cargo test -q tool_output_store
cargo check --features experimental-api-server -q
```

## 6. Phase 2: Provider Productization

### opencode reference

opencode models providers as product data:

- provider config includes API settings, request overrides, model metadata,
  cost, limits, variants, capabilities, and disabled state;
- provider connection source is visible in UI (`env`, `api`, `config`,
  `custom`);
- provider tests cover model status, transforms, provider-specific request
  behavior, and recorded/golden protocol behavior;
- settings UI lets the user connect, disconnect, inspect, and manage providers.

### Priority Agent status

Priority Agent has provider registry, protocol facts, provider runtime profile,
retry policy, health JSONL, usage ledger, provider status commands, and desktop
status fields. The experimental API now exposes a product-shaped provider
status DTO, but deeper provider productization remains:

- provider/model capability data is not persisted as a first-class product
  table;
- provider health JSONL can be reflected in the API's latest matching provider
  status, but it is not yet surfaced as a desktop/TUI status history;
- slow-tail and timeout settings are still spread across env/config/runtime
  structs;
- model max output/context/cost/cached-token cost are not consistently visible
  in settings/status;
- provider transform tests exist in places, but there is not yet an obvious
  provider certification matrix for DeepSeek/OpenAI-compatible/MiniMax/Kimi.

### Work Items

1. Add `ProviderProductStatus` DTO:
   - provider id and label;
   - model id and display name;
   - connection source (`env`, `config`, `desktop_settings`, `runtime`,
     `custom`);
   - configured/active/disabled;
   - base URL host;
   - protocol family;
   - supports streaming tool calls;
   - non-streaming fallback requirement;
   - context limit;
   - output limit;
   - configured max output;
   - cost input/output/cache read/cache write when known;
   - latest health result;
   - latest timeout/failure category;
   - last request latency and retry count when available.

   Status 2026-06-07: base DTO and `/api/provider/status` page are implemented.
   Remaining: real cost fields, retry count from the runtime ledger, persisted
   model catalog data, and desktop/TUI consumption.

2. Persist recent provider health into a queryable store:
   - either SQLite `provider_health_runs`;
   - or keep JSONL as source but add indexed summary projection;
   - expose latest status per provider/model.

3. Productize timeout config:
   - one effective config snapshot:
     `provider.timeout.request_secs`,
     `provider.timeout.stream_idle_secs`,
     `provider.timeout.slow_warning_secs`,
     `provider.retry.max_attempts`;
   - include source (`default`, `project_config`, `env`) for each value;
   - expose in `/provider status --json`, desktop status, and diagnostic export.

4. Add provider certification tests:
   - request transform golden test per provider family;
   - tool-call streaming/non-streaming behavior;
   - timeout classification;
   - usage extraction;
   - cached-token extraction;
   - retry/no-retry after local side effects.

5. Add provider settings UI:
   - connected providers list;
   - source badges (`env`, `config`, `desktop`);
   - active model;
   - health status;
   - last failure;
   - effective timeout;
   - link to diagnostics folder/export.

6. Keep provider status honest:
   - show "unverified" when no health check has run;
   - show "configured but unavailable" when env/config exists but preflight
     fails;
   - show "runtime using model X" separately from "settings selected model Y".

### Acceptance

- A user can answer: "Which provider/model am I actually using?"
- A user can answer: "Why is it slow?"
- A user can answer: "Is this provider configured from env or project config?"
- DeepSeek/OpenAI-compatible/MiniMax/Kimi protocol behavior has focused tests.
- Provider failures are classified as provider/model/environment/framework
  without reading raw logs.

### Suggested Gates

```bash
cargo test -q provider
cargo test -q provider_health
cargo test -q usage_ledger
cargo test -q diagnostics
corepack pnpm --dir apps/desktop exec tsc --noEmit
```

## 7. Phase 3: Desktop/TUI Product UI

### opencode reference

opencode's app has visible product surfaces for:

- server and sync status popovers;
- connected providers and provider settings;
- custom provider dialogs;
- model selection and model tooltips;
- context usage and context breakdown;
- session side panels, file tabs, terminal panels, history, and titlebar state;
- prompt-input attachments, history, slash popovers, and submit behavior.

### Priority Agent status

Priority Agent desktop already has a functional agent surface: transcript,
status bar, provider/model picker, settings drawer, diagnostics panel, trace
drawer, tool-output drawer, permission cards, workbench drawer, context detail,
and persisted session reload. TUI has many slash/status surfaces. Remaining
product gaps:

- desktop provider status is present but not yet a full provider settings and
  health panel;
- desktop does not yet expose the same permission explanation depth as TUI;
- desktop queued/pending input status is not as visible as TUI;
- diagnostic export is powerful but not easy to trigger, inspect, and explain
  from desktop;
- context usage is available but should become a stable, glanceable panel;
- revert state is visible, but a richer "revert this turn" / history UX should
  remain discoverable.

### Work Items

1. Desktop status popover:
   - provider/model active status;
   - provider health;
   - runtime running/idle/queued;
   - permission mode;
   - context usage;
   - tool-output retention/policy;
   - last diagnostic export link.

2. Provider settings panel:
   - provider list with source badges;
   - current model and available models;
   - connect/disconnect guidance;
   - health check button;
   - effective timeout display;
   - error explanation when provider is configured but unavailable.

3. Diagnostics panel product pass:
   - export latest diagnostic;
   - show session id, provider, model, cost, cache, retries;
   - show failed tools and failure owner;
   - show revert history;
   - open diagnostics folder;
   - copy redacted JSON.

4. Permission explanation UI:
   - show winning rule;
   - show scope/source;
   - show matched shell paths and risk reason;
   - show recommended recovery tool (`file_edit`, `file_patch`, etc.);
   - keep high-risk hard blocks visually distinct from ordinary permission
     prompts.

5. Queue/running state UI:
   - show pending input count;
   - show whether input is queued or steering current turn;
   - expose cancel queued input when safe;
   - keep duplicate-submit state visible.

6. Context and tool-output UI:
   - context usage breakdown;
   - large output index by session;
   - page/search stored output;
   - show active project-level `tool_output` policy.

7. Revert UI polish:
   - visible per-assistant-turn revert action;
   - revert history panel;
   - dimmed reverted parts already exist, but add clearer affordance;
   - show unrevert availability when snapshot still exists.

### Acceptance

- A user can diagnose common failures from desktop without reading terminal
  logs.
- Desktop and TUI show consistent provider, permission, session, and tool-output
  state.
- UI state survives session reload and app restart.
- Status UI uses DTOs from `apps/desktop/src/runtime/desktopApi.ts`, not raw
  event payload guesses.

### Suggested Gates

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts
corepack pnpm --dir apps/desktop test:ui-smoke
cd apps/desktop/src-tauri && cargo test -q
cargo test -q desktop
```

## 8. Phase 4: Config And Permission Productization

### opencode reference

opencode's config v2 work separates schema groups and uses typed config
modules for provider, tool output, permissions, skills, references, formatter,
LSP, attachments, and other domains. Saved permissions are project-scoped
records with explicit action/resource fields.

### Priority Agent status

Priority Agent has config schema support, project-level tool-output policy,
provider env/config, plugin trust mode, memory provider config, and permission
rules. Remaining gaps:

- project config does not yet cover provider timeout, provider model limits,
  formatter/LSP overrides, or desktop-visible provider settings in one
  coherent schema;
- saved permission rules are not yet as clearly project-scoped/auditable as
  opencode's action/resource records;
- config editing is mostly slash/tool based, not product UI based.

### Work Items

1. Add config schema groups:
   - `provider.*`;
   - `provider.timeout.*`;
   - `tool_output.*`;
   - `permissions.*`;
   - `desktop.*`;
   - `lsp.*`;
   - `formatter.*` only if current code has a consumer.

2. Add config source reporting:
   - default;
   - user config;
   - project config;
   - env;
   - runtime override.

3. Persist project-scoped permission rules:
   - rule id;
   - project id/path;
   - action;
   - resource;
   - effect;
   - source;
   - created_at;
   - optional expiry.

4. Add permissions management UI:
   - list current project rules;
   - remove rule;
   - explain why a command matched a rule;
   - show high-risk rules that remain non-overridable.

5. Add config tests:
   - project config override;
   - env final override;
   - invalid config diagnostics;
   - redacted export.

### Acceptance

- A user can answer: "Where did this setting come from?"
- A user can answer: "Which permission rule allowed this?"
- Project config is deterministic and safe to share.
- Secrets are redacted in exports.

### Suggested Gates

```bash
cargo test -q config
cargo test -q permissions
cargo test -q action_review
cargo test -q diagnostics
```

## 9. Phase 5: Product Soak And Release Gates

This phase turns the above product surfaces into release confidence.

### Work Items

1. Define a real-task soak suite:
   - small backend API task;
   - frontend UI task;
   - multi-file Rust fix;
   - failing test repair;
   - long-output test log;
   - provider slow-tail run;
   - desktop reload after tool activity;
   - revert/unrevert task;
   - permission denial and recovery task.

2. Run the same suite through:
   - CLI;
   - TUI expect mode;
   - desktop mocked/smoke mode;
   - at least one real provider path when credentials are available.

3. Produce daily baseline report:
   - pass/fail/partial;
   - failure owner;
   - provider/model;
   - usage/cost;
   - tool rounds;
   - cache stats;
   - screenshots/log links for desktop failures.

4. Make release blockers explicit:
   - data loss;
   - false verified closeout;
   - unrecoverable desktop reload;
   - provider timeout with no visible diagnosis;
   - permission hard gate bypass;
   - API schema drift.

### Acceptance

- One full daily baseline can run without manual log archaeology.
- A failure report says whether the issue is framework, provider/model, test
  harness, or environment.
- UI/API/provider status is enough to debug the common failure classes.

### Suggested Gates

```bash
cargo fmt --check
cargo check -q
cargo check --features experimental-api-server -q
cargo test -q
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop test:ui-smoke
bash scripts/workflow-production-gates.sh
```

## 10. Recommended Order

Do this in small, gateable slices:

1. API DTO module and session/tool-output/provider read routes.
2. Provider product status DTO plus health summary projection.
3. Desktop status popover and provider settings panel.
4. Permission/config product UI and project-scoped saved permissions.
5. Real-task soak suite and release baseline report.

The reason for this order is dependency flow: UI should consume stable DTOs;
provider UI should consume stable provider status; release soak should validate
the product surfaces rather than raw internal traces.

## 11. Definition Of Done

This productization phase is complete when:

- API contracts are documented, tested, and feature-gated.
- Desktop and TUI consume shared DTO concepts for session/provider/tool-output
  status.
- Provider health, timeout, usage, cache, and model capability are visible from
  normal product UI.
- Project config can explain active values and redacts secrets.
- Permission decisions can be explained and audited by project.
- One real-task soak baseline can run and produce actionable failure ownership.

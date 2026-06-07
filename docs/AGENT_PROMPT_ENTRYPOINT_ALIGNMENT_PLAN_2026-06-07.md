# Agent Prompt Entrypoint Alignment Plan

Date: 2026-06-07

Status: proposed implementation plan

Reference code reviewed:

- opencode:
  - `/Users/georgexu/Downloads/opencode-dev/packages/server/src/groups/v2/session.ts`
  - `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/server/routes/instance/httpapi/groups/session.ts`
  - `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/server/routes/instance/httpapi/handlers/session.ts`
  - `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/prompt.ts`
  - `/Users/georgexu/Downloads/opencode-dev/packages/app/src/components/prompt-input/submit.ts`
  - `/Users/georgexu/Downloads/opencode-dev/AGENTS.md`
- priority-agent:
  - `src/api/routes.rs`
  - `src/api/state.rs`
  - `src/desktop_runtime/mod.rs`
  - `src/engine/runtime_controller.rs`
  - `src/engine/streaming.rs`
  - `src/shell.rs`
  - `src/tui/app.rs`
  - `docs/API_CONTRACT_PROVIDER_UI_NEXT_STEPS_2026-06-07.md`
  - `docs/api/session_schema.md`

## 1. Problem

The product currently has two user-facing LLM lanes that can be confused:

1. `POST /api/chat`
   - Calls the configured provider directly through `ApiState::chat`.
   - Does not enter `RuntimeController`.
   - Does not run the full agent loop.
   - Does not use tools, checkpoints, permission flow, closeout proof, or the
     durable `session_events` / `session_parts` projection.
   - Now returns `execution_kind: "provider_chat"` and `full_agent: false`, but
     the route name still looks like a normal chat/agent entry.

2. Full-agent paths
   - CLI shell uses `RuntimeController::submit_stream_turn`.
   - TUI uses `RuntimeController`.
   - Desktop full turns use `DesktopRuntime::run_full_turn`, which delegates to
     `RuntimeController`.
   - These are the real programming-agent paths.

This split is acceptable only if it is explicit. It is not acceptable if a user,
frontend, SDK, eval, or future integration treats provider chat as the primary
agent conversation path.

The desired product behavior is:

- every formal user task enters the full-agent session prompt path;
- lightweight/direct provider calls exist only for named auxiliary use cases;
- routing is decided by the entrypoint, not by a growing pile of heuristic
  "is this simple enough?" rules.

## 2. What opencode Does Better

opencode's main user-facing prompt design is session-centered.

Important observed patterns:

- HTTP v2 route:
  - `POST /api/session/:sessionID/prompt`
  - payload includes prompt identity and delivery controls.
  - description says it durably admits a session input and schedules agent-loop
    execution unless explicitly told not to resume.
- App prompt input:
  - submits through `client.session.promptAsync`.
  - not through a plain provider chat endpoint.
- Session prompt service:
  - `SessionPrompt.prompt` creates a durable user message, touches the session,
    applies permission overrides, and then calls the loop unless `noReply` is
    requested.
- V2 notes in opencode `AGENTS.md`:
  - durable prompt admission is separate from model execution;
  - session execution is process-global and session-ID based;
  - prompt delivery vocabulary is explicit;
  - one provider turn uses one explicit `llm.stream(request)` call;
  - projected history is reloaded before durable continuation.

The practical product lesson:

opencode does not make a generic provider chat endpoint the normal user
conversation path. The user-facing path is session prompt first, agent runner
second, provider stream third.

It can still have internal LLM calls for titles, summaries, compaction, model
work, or helper behavior, but those are not presented as the main agent
conversation.

## 3. Current priority-agent State

### Already Good

- `RuntimeController` exists and is already the intended full-agent boundary.
- CLI shell full turns go through `RuntimeController`.
- TUI full turns go through `RuntimeController`.
- Desktop full turns go through `DesktopRuntime::run_full_turn`, which delegates
  to `RuntimeController`.
- Desktop lightweight lane is explicitly non-agent in `DesktopRuntime`.
- `/api/chat` now labels itself as `execution_kind: "provider_chat"` and
  `full_agent: false`.
- `POST /api/sessions/:id/prompt` now exists as a typed `501` boundary with:
  - `execution_kind: "full_agent_turn"`;
  - `accepted: false`;
  - `agent_runtime_entrypoint: "RuntimeController"`;
  - no fake diagnostic/events.

### Main Gap

The HTTP API has the right full-agent route name, but it is not wired to the
real runtime yet.

`ApiState` currently owns:

- provider;
- model;
- tool registry;
- session store;
- config;
- audit tracker;
- LSP/worktree managers.

It does not own or receive a `RuntimeController` or `StreamingQueryEngine`.

That means a direct implementation inside `session_prompt_handler` would be
tempting but dangerous: it could instantiate a fresh runtime that does not share
the real frontend session/history/permission/checkpoint path. That would create
a fake API agent lane.

## 4. Target Policy

Adopt an opencode-like policy:

### Formal User Task Entrypoints

These must enter the full-agent path:

- CLI normal user input;
- TUI main input;
- desktop main prompt input;
- HTTP `POST /api/sessions/:id/prompt`;
- future SDK `session.prompt` / `session.prompt_async`;
- eval harnesses that claim to test the real product.

These paths should all converge on:

```text
Session Prompt Admission -> RuntimeController -> StreamingQueryEngine -> Tool Loop
```

### Auxiliary LLM Entrypoints

These may call the provider directly:

- session title generation;
- summarization / compaction;
- memory extraction / proposal review;
- provider health checks;
- model capability checks;
- internal classification if still needed;
- explicitly named desktop side question;
- explicitly named API provider reply/debug route.

These paths must be marked:

- `full_agent: false`;
- `execution_kind: "provider_chat"` or a more specific non-agent kind;
- no tools;
- no checkpoint claims;
- no verified closeout claims;
- no "agent completed the task" language.

### Avoid Heuristic Routing

Do not make the product depend on runtime guessing whether a user message is
"simple enough" for a light lane.

Instead:

- the main input box means full-agent;
- a side-question button means lightweight;
- an internal service call means auxiliary;
- an HTTP route name determines the lane.

This keeps behavior stable and avoids a growing, fragile classification system.

## 5. Proposed API Shape

### Primary Route

```text
POST /api/sessions/:id/prompt
```

This is the primary HTTP user prompt route.

Recommended request:

```json
{
  "message": "string",
  "agent_mode": "normal | plan | review | optional",
  "delivery": "run | admit_only | queue | optional",
  "stream": "bool | optional",
  "idempotency_key": "string | optional"
}
```

Recommended non-streaming response:

```json
{
  "session_id": "string",
  "execution_kind": "full_agent_turn",
  "accepted": true,
  "turn_id": "string",
  "status": "completed | failed | partial | not_verified | cancelled",
  "events_written": "usize",
  "latest_part_index": "i64 | null",
  "diagnostic": "DiagnosticExportDto | null",
  "agent_runtime_entrypoint": "RuntimeController",
  "error": "string | null"
}
```

For the current transition period, typed `501` is acceptable only as an honest
blocker. It should not be treated as feature completion.

### Async Route

Later, add:

```text
POST /api/sessions/:id/prompt_async
```

This should admit the input, wake the runner, and return quickly.

This is closer to opencode's `promptAsync` route and desktop behavior.

### Legacy Provider Route

Keep direct provider chat only as an explicitly named compatibility route:

```text
POST /api/provider-chat
```

or:

```text
POST /api/llm/reply
```

During migration:

- keep `POST /api/chat`;
- mark it deprecated in docs;
- return a deprecation field or warning field if useful;
- route docs and frontend code away from it.

Do not let `/api/chat` become the recommended user task route.

## 6. Implementation Plan

### Slice 1: Documentation And Route Vocabulary

Goal:

Make the product vocabulary unambiguous before wiring more runtime.

Changes:

- Update API docs to say:
  - `POST /api/sessions/:id/prompt` is the formal user-task route;
  - `POST /api/chat` is provider-chat compatibility only;
  - lightweight lanes are non-agent lanes.
- Update `src/api/mod.rs` endpoint comments and startup logs:
  - log `POST /api/sessions/:id/prompt` as the future/primary agent route;
  - log `/api/chat` as provider-chat compatibility.
- Update `docs/PROJECT_MAP.md`:
  - main prompt path = `RuntimeController`;
  - provider-chat path = auxiliary only.
- Update `docs/API_CONTRACT_PROVIDER_UI_NEXT_STEPS_2026-06-07.md`:
  - keep typed 501 as an honest blocker;
  - make real controller injection the next hard requirement.

Tests:

```bash
cargo fmt --check
cargo test -q api::routes --features experimental-api-server
```

Done when:

- docs no longer imply `/api/chat` is a user-task route;
- route contract test still proves `/api/chat` is `full_agent: false`;
- route contract test proves `/api/sessions/:id/prompt` is the full-agent
  boundary, even if still typed 501.

### Slice 2: Add An API Runtime Handle

Goal:

Allow API routes to call the real full-agent runtime without constructing a
fake runtime inside the handler.

Recommended shape:

```rust
pub struct ApiState {
    ...
    pub runtime_controller: Option<Arc<RuntimeController>>,
}
```

or better:

```rust
pub trait ApiAgentRuntime: Send + Sync {
    async fn submit_prompt(&self, input: ApiSessionPromptInput)
        -> anyhow::Result<ApiSessionPromptOutcome>;
}
```

Use a trait if tests need a deterministic fake and if desktop/API injection
should stay decoupled from `RuntimeController` internals.

Rules:

- No API handler should call `StreamingQueryEngine::new(...)` directly.
- No API handler should initialize a new provider/tool/runtime stack for a
  formal user task.
- If no runtime handle exists, return the existing typed 501.

Code entry points:

- `src/api/state.rs`
- `src/api/routes.rs`
- `src/api/routes/contract_tests.rs`
- `src/engine/runtime_controller.rs`

Tests:

```bash
cargo test -q api::routes --features experimental-api-server
cargo check --features experimental-api-server -q
```

Done when:

- `ApiState` can hold an optional full-agent runtime boundary;
- existing API server still starts without it;
- tests can inject a fake runtime and receive a full-agent response;
- missing runtime still returns typed 501.

### Slice 3: Implement Session Prompt Admission Semantics

Goal:

Make HTTP prompt submission session-first, not provider-first.

Minimum behavior:

- validate session exists or create/adopt it according to product decision;
- bind runtime to `session_id`;
- restore compacted history if required;
- submit the message through `RuntimeController`;
- collect stream events until terminal status for non-streaming mode;
- write/refresh durable events and parts through existing mirror/projection;
- return latest diagnostic summary.

Important details:

- Reusing an explicit request id should be idempotent where possible.
- If full idempotency is too large, start with a documented `idempotency_key`
  placeholder and no retry guarantee.
- `delivery: "admit_only"` can be added later; do not fake it.
- `delivery: "queue"` requires a run coordinator; defer if no coordinator exists.

Code entry points:

- `src/api/routes.rs`
- `src/api/state.rs`
- `src/engine/runtime_controller.rs`
- `src/engine/streaming.rs`
- `src/session_store/event_mirror.rs`
- `src/session_store/session_parts.rs`
- `src/session_store/mod.rs`

Tests:

```bash
cargo test -q api::routes --features experimental-api-server
cargo test -q runtime_controller
cargo test -q session_parts
cargo check --features experimental-api-server -q
```

Done when:

- `POST /api/sessions/:id/prompt` with a fake runtime returns
  `accepted: true`;
- the handler never calls direct provider chat;
- response includes terminal status and stable metadata;
- session parts/events are visible after the call in tests.

### Slice 4: Rename Or Deprecate `/api/chat`

Goal:

Remove the ambiguous public name.

Migration path:

1. Add explicit route:

   ```text
   POST /api/provider-chat
   ```

2. Make `/api/chat` a legacy alias to the same handler.
3. Add response metadata:

   ```json
   {
     "execution_kind": "provider_chat",
     "full_agent": false,
     "deprecated_route": "/api/chat",
     "replacement_route": "/api/provider-chat"
   }
   ```

4. Update docs and examples to use:
   - `/api/sessions/:id/prompt` for agent work;
   - `/api/provider-chat` only for explicit provider reply/debug.

Tests:

```bash
cargo test -q api::routes --features experimental-api-server
```

Done when:

- no docs recommend `/api/chat` for user tasks;
- both `/api/chat` and `/api/provider-chat` return `full_agent: false`;
- contract tests prevent accidental relabeling.

### Slice 5: Desktop/TUI/Product UX Cleanup

Goal:

Make entrypoint meaning visible to the user.

Desktop:

- main prompt input uses full-agent lane;
- side question UI, if kept, is visibly separate;
- side question response should show it is a lightweight reply, not a verified
  agent run;
- no "done", "verified", "changed files", or revert controls for lightweight
  replies.

TUI/CLI:

- normal input remains full-agent;
- any future direct provider helper command should be explicit, for example:

  ```text
  /provider-chat explain this term
  ```

HTTP/API:

- primary SDK sample should create/adopt a session then call session prompt;
- provider-chat sample should be in a debugging/advanced section.

Tests:

```bash
cargo test -q turn_ingress
cargo test -q runtime_controller
cargo test -q api::routes --features experimental-api-server
corepack pnpm --dir apps/desktop exec tsc --noEmit
```

Done when:

- UI and docs do not present lightweight replies as agent turns;
- no frontend uses `/api/chat` for formal agent tasks.

### Slice 6: Remove Heuristic Dependency

Goal:

Stop relying on broad automatic "simple vs complex" classification for formal
user input.

Keep deterministic entrypoint routing:

- main prompt -> full-agent;
- side question -> lightweight;
- internal service -> auxiliary provider call.

Review these files:

- `src/engine/turn_ingress.rs`
- `src/desktop_runtime/mod.rs`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/app/runEventState.ts`
- `src/engine/task_context.rs`
- `src/engine/lightweight_planner.rs`

Expected outcome:

- lightweight planning can remain for internal task context and UI hints;
- it should not decide whether the main user task bypasses the agent runtime.

Tests:

```bash
cargo test -q turn_ingress
cargo test -q lightweight_planner
cargo test -q runtime_controller
```

Done when:

- the full-agent route is selected by entrypoint, not model/heuristic judgment;
- lightweight lane is impossible to invoke accidentally from the main task path.

## 7. Risks And Guardrails

### Risk: Token Cost Increases

If all formal prompts go full-agent, simple questions may cost more.

Mitigation:

- keep explicit side-question lane;
- keep compact tool schema exposure by route/role;
- keep output caps;
- use context trimming and usage ledger;
- do not expose unnecessary tools for all turns.

### Risk: Slower Simple Replies

Full-agent path has more runtime setup than provider chat.

Mitigation:

- accept this for formal user tasks;
- provide explicit lightweight side question for casual Q&A;
- optimize runtime startup and provider slow-tail separately.

### Risk: Fake API Runtime

The largest implementation risk is accidentally constructing a separate runtime
inside the API handler.

Guardrail:

- API handler must receive an injected runtime boundary.
- If runtime is absent, return typed 501.
- Tests should assert no direct provider chat call happens for
  `/api/sessions/:id/prompt`.

### Risk: Misleading UI

Lightweight replies can look like agent work if rendered in the same transcript.

Guardrail:

- render lightweight replies as non-agent side replies;
- do not attach checkpoint/revert/verified controls;
- record `execution_kind`.

### Risk: Breaking Existing API Consumers

Existing consumers may call `/api/chat`.

Mitigation:

- keep `/api/chat` as legacy alias temporarily;
- add `/api/provider-chat`;
- mark deprecation in docs and response metadata;
- do not remove `/api/chat` until a release boundary.

## 8. Recommended Execution Order

1. Document route vocabulary and update API startup logs.
2. Add optional `ApiAgentRuntime` / `RuntimeController` handle to `ApiState`.
3. Add fake-runtime route tests proving `/api/sessions/:id/prompt` can return
   `accepted: true`.
4. Wire real `RuntimeController` for API server mode.
5. Add `/api/provider-chat` and deprecate `/api/chat`.
6. Update desktop/API examples to use session prompt for formal tasks.
7. Review and reduce heuristic routing dependency.
8. Run a real task soak through CLI/TUI/desktop/API to prove the same runtime
   semantics.

## 9. Definition Of Done

This plan is done when:

- all formal user prompt entrypoints converge on `RuntimeController`;
- `POST /api/sessions/:id/prompt` is a real agent route, not typed 501;
- `/api/chat` is no longer documented as a normal user prompt route;
- direct provider calls are named auxiliary lanes;
- tests prove provider-chat cannot be mistaken for full-agent execution;
- session events/parts/diagnostics are written for HTTP agent prompts;
- desktop/TUI/API share the same entrypoint vocabulary;
- simple/lightweight behavior exists only through explicit side-question or
  internal service routes.

## 10. Immediate Next Slice

The next code slice should be:

**Add `ApiAgentRuntime` injection to `ApiState` and keep missing-runtime typed
501 behavior.**

Why this first:

- it avoids fake runtime construction;
- it gives tests a clean fake runtime;
- it creates the attachment point for real `RuntimeController`;
- it can be implemented without changing provider/tool/runtime internals.

Suggested first test:

- construct `ApiState` with a fake `ApiAgentRuntime`;
- call `POST /api/sessions/test/prompt`;
- assert:
  - status `200`;
  - `execution_kind == "full_agent_turn"`;
  - `accepted == true`;
  - `agent_runtime_entrypoint == "RuntimeController"`;
  - fake runtime received `session_id == "test"`;
  - fake runtime received the exact message;
  - no provider-chat handler was used.

Then keep the existing missing-runtime test:

- construct `ApiState` without runtime;
- call `POST /api/sessions/test/prompt`;
- assert typed `501`.


# Reasonix-Style Product Hardening Plan

Date: 2026-06-03

## Why This Document Exists

Priority Agent has grown into a deeper and stricter coding agent than Reasonix in
several places: validation proof, memory governance, permission boundaries,
provider slow-tail telemetry, runtime traces, and repair-loop evidence are all
more ambitious here.

The gap is not mainly missing features. The gap is product shape:

- Reasonix feels smaller because its control boundaries are cleaner.
- Reasonix is easier to explain because memory, tools, config, and provider
  behavior have plain user-facing models.
- Reasonix is more cache-oriented because it treats the system prompt prefix,
  tool schemas, and loaded memory as a stable product contract.
- Priority Agent currently has stronger internal mechanisms, but they are still
  spread across many runtime, TUI, desktop, memory, provider, and tool modules.

This plan defines the next development stage: do not add broad new features by
default. Instead, make the existing product more stable, simpler to operate, and
easier to reason about, while preserving the hard constraints that make Priority
Agent valuable.

## Reference Snapshot

Reasonix source reviewed from:

- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/REASONIX.md`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/README.md`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/control/controller.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/control/auto_plan.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/agent.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/session.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/tool/tool.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/memory/memory.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/memory/store.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/provider/provider.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/provider/retry.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/config/config.go`

Priority Agent source reviewed from:

- `src/engine/query_engine.rs`
- `src/engine/streaming.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/services/api/mod.rs`
- `src/services/api/retry.rs`
- `src/services/api/provider_protocol.rs`
- `src/memory/mod.rs`
- `src/memory/manager.rs`
- `src/memory/extraction.rs`
- `src/tools/mod.rs`
- `src/tui/app.rs`
- `src/tui/screens/main_screen.rs`
- `docs/PROJECT_STATUS.md`

Quick scale check:

- Priority Agent: about 464 Rust files and about 249k Rust lines.
- Reasonix: about 422 Go files and about 77k Go lines.
- Priority Agent tool module surface: about 45 top-level tool modules under
  `src/tools`.
- Reasonix built-in tool surface: about 24 Go implementation files under
  `internal/tool/builtin`.

The numbers are not a quality score. They show why Priority Agent needs product
hardening and sharper boundaries before more feature growth.

## What Reasonix Does Better

### 1. One Controller Boundary For All Frontends

Reasonix centers product behavior behind one transport-agnostic controller. The
chat TUI, HTTP/SSE serve path, and desktop app call into the same control layer.
The agent loop does not need to know the details of each frontend.

Priority Agent has the right direction with `StreamingQueryEngine` and
`DesktopRuntime`, but practical ownership is still spread across:

- `src/engine/query_engine.rs`
- `src/engine/streaming.rs`
- `src/engine/conversation_loop/*`
- `src/tui/app.rs`
- `apps/desktop/src-tauri/src/*`
- `src/session_store/mod.rs`

Problem:

- TUI/desktop runtime state can still know too much about provider, memory,
  session, and trace details.
- Product behavior can be fixed in one frontend path but not automatically land
  in another.
- Test coverage has to chase multiple entrypoints instead of one product
  facade.

Target:

- A single runtime facade should own user turn submission, cancellation,
  runtime diagnostics, session persistence, provider status, memory status,
  approvals, and closeout state.
- CLI/TUI/desktop should render runtime events and submit user actions, not
  duplicate runtime policy.

### 2. Cache-Stable Prefix Is A First-Class Product Contract

Reasonix treats the base prompt, tool schemas, and loaded memory as a stable
prefix. Dynamic data is pushed into the current turn tail instead of mutating the
prefix mid-session.

Priority Agent has made progress:

- prompt context has stable-prefix reporting;
- dynamic retrieval is separated into context zones;
- provider slow-tail telemetry now uses runtime diagnostics instead of prompt
  rules;
- memory proposals are review-oriented by default.

Remaining risk:

- Some context assembly, memory recall, skill/tool exposure, and route policy
  paths still need explicit cache-stability assertions.
- Tool schema order and enabled tool set must be stable for equivalent routes.
- Runtime diagnostics must never leak back into the stable prefix.

Target:

- A user-facing invariant: "The session prefix is fixed after turn bootstrap;
  dynamic hints and newly saved memory apply through turn-tail notes or next
  session reload."

### 3. Tool Surface Is Smaller And Easier To Explain

Reasonix has a smaller default built-in set and leans on config/plugins/MCP for
extension. Its tool registry has a simple contract: built-ins register, plugin
tools join the per-run registry, schemas are exported in stable order, and
read-only tools can be parallelized.

Priority Agent has a richer tool ecosystem, but the default surface is wide:

- file, bash, grep, glob, git, GitHub, browser, notebook, memory, MCP, project,
  task, agent, LSP, symbol, worktree, remote, telemetry, team, workbench,
  plugin, config, share, voice, and more.

Problem:

- More tools increase prompt schema size and cache fragility.
- More tool categories make permission and evidence behavior harder to explain.
- Long-tail tools become product promises even when they are rarely used.

Target:

- Keep core coding tools first-class.
- Move optional or situational tools behind route-scoped exposure, config, MCP,
  or plugin activation.
- Make the default tool contract small enough for a user to understand.

### 4. Configuration Is More Productized

Reasonix exposes provider selection, tools, permissions, sandbox, plugins,
skills, LSP, statusline, and proxy behavior in one TOML-driven product surface.
Secrets stay in environment variables, but behavior lives in config.

Priority Agent still has many runtime switches in environment variables or code
defaults:

- provider retry and timeout settings;
- memory use/generate/active modes;
- auto memory write policies;
- route-scoped tool exposure;
- workflow contract targeting;
- active-memory prototype;
- skill trust and allowlist behavior;
- daily gate output paths.

Environment variables are useful for experiments, but they are not enough for a
commercial-grade product surface.

Target:

- Keep env overrides for developer/debug usage.
- Add a stable project/user config layer for product behavior.
- `/config` or `/doctor` should explain the effective configuration and where
  each value came from.

### 5. Memory UX Is Simpler

Reasonix memory is easy to explain:

- hierarchical memory docs;
- a per-project memory store;
- one fact per Markdown file;
- `MEMORY.md` as an index;
- `remember` and `forget` as model-visible tools;
- saved index loads into the prefix on the next session;
- current-session changes apply through a queued turn-tail note.

Priority Agent memory is more advanced:

- typed records;
- proposal review queue;
- safety scan;
- quality scoring;
- provider lifecycle;
- local FTS index;
- active memory prototype;
- review-only default policy;
- explicit apply/reject flows.

Problem:

- The internal design is strong, but the product story is hard to explain.
- It is not yet obvious to a normal user which memory files are loaded, when a
  memory applies, and whether a newly proposed memory is active now or only
  after review.

Target:

- Present memory as three simple layers:
  - Stable memory prefix: project/user memory docs plus an index of accepted
    durable facts.
  - Turn-tail memory updates: notes about memory changes made during this
    session.
  - On-demand memory reads: when the index says a specific fact is relevant,
    the agent reads the source file before relying on it.

The existing safety/proposal machinery can remain underneath, but the user
model should be as simple as Reasonix.

### 6. Distribution And First-Run Path Are Clearer

Reasonix has a clear install story:

- npm package wrapping a native binary;
- Homebrew;
- single binary build;
- setup wizard;
- example config.

Priority Agent currently focuses more on local development, TUI, desktop, and
runtime quality than a clean first-run product path.

Target:

- A single documented "install/open/run first task" path.
- A small setup wizard or setup command that writes config, validates provider
  auth, checks tools, and creates project memory if missing.
- Daily baseline should test this first-run path.

## Where Priority Agent Is Stronger

These strengths should be preserved, not simplified away:

- deterministic validation and closeout proof;
- permission and checkpoint boundaries;
- provider-protocol normalization for OpenAI-compatible, Kimi, and MiniMax;
- provider slow-tail telemetry, retry, timeout, cancellation, and TUI status;
- memory safety, proposal review, evidence, and lifecycle diagnostics;
- workflow trace and daily/live eval evidence;
- read-before-edit and exact duplicate loop hardening;
- focused repair and honest `not_verified`/`partial` outcomes.

The plan is not to become a clone of Reasonix. The plan is to learn from its
product shape while keeping Priority Agent's deeper correctness constraints.

## Product Principle For The Next Stage

Priority Agent should become:

> A narrow, deep, personal coding agent with a small default interface, stable
> cached context, explicit evidence, and a single runtime spine shared by CLI,
> TUI, desktop, and eval.

Do not add a new feature unless one of these is true:

- it reduces user-visible complexity;
- it closes a real product reliability gap;
- it moves behavior behind a cleaner boundary;
- it improves the daily baseline's ability to catch regressions;
- it preserves an existing hard safety/proof contract.

## Development Plan

### Phase 1: Runtime Facade Consolidation

Goal:

Make one product runtime facade the owner of turn submission, cancellation,
diagnostics, session state, provider lifecycle, memory lifecycle, approvals, and
closeout status.

Work items:

1. Inventory all current entrypoints:
   - CLI normal mode;
   - TUI compatibility mode;
   - desktop runtime;
   - eval runner;
   - headless agent-run paths.
2. Define a `RuntimeFacade` or equivalent contract with:
   - `submit_turn`;
   - `cancel_turn`;
   - `snapshot_state`;
   - `approve_tool`;
   - `answer_question`;
   - `switch_session` or session resume;
   - event stream subscription;
   - effective config snapshot.
3. Move frontend-specific state consumption into adapters:
   - TUI renders facade events;
   - desktop renders facade events;
   - eval records facade events;
   - CLI prints facade events.
4. Keep `ConversationLoop` focused on model/tool loop behavior.
5. Keep provider/memory/tool policy decisions outside frontend code.

Acceptance criteria:

- TUI and desktop use the same provider request lifecycle events.
- Tool approval behavior has one owner.
- Session persistence path is not duplicated between desktop and TUI.
- A single dogfood test can exercise the same runtime spine through at least
  TUI/headless.

Suggested validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q runtime_timeouts
cargo test -q tui
cargo test -q closeout
bash scripts/runtime-entrypoint-smoke.sh
bash scripts/tui-dogfood-test.sh
```

### Phase 2: Cache-Stability Audit And Contract Tests

Goal:

Turn cache stability from a design intention into a testable product contract.

Work items:

1. Add or refresh a cache-stability matrix for:
   - same session, repeated user turns;
   - plan mode toggles;
   - approvals;
   - provider slow warning;
   - memory proposal creation;
   - memory proposal acceptance;
   - route-scoped tool exposure;
   - skill availability;
   - active-memory retrieval.
2. Assert that stable prefix fingerprints do not change when only turn-tail
   runtime state changes.
3. Assert that equivalent tool sets export schemas in stable order.
4. Assert that dynamic retrieval zones are not included in the stable prefix
   fingerprint.
5. Document exactly which actions intentionally bust prefix cache:
   - config change;
   - accepted memory change loaded into the next session;
   - enabled tool/plugin change;
   - base instruction file change.

Acceptance criteria:

- There is a dedicated cache-stability test lane.
- `/doctor` or trace output can explain the current stable prefix fingerprint
  and dynamic context zones.
- No runtime diagnostic is prompt-injected as stable prefix content.

Suggested validation:

```bash
cargo test -q prompt_context
cargo test -q cache_stability
cargo test -q route_scoped_tools
cargo test -q memory
```

### Phase 3: Default Tool Surface Slimming

Goal:

Make the default model-visible tool surface feel like a focused coding agent,
not an inventory of every possible capability.

Work items:

1. Classify all tools into product tiers:
   - Core default coding tools;
   - default diagnostic tools;
   - route-scoped tools;
   - config-enabled tools;
   - plugin/MCP-like optional tools;
   - deprecated or hidden tools.
2. Proposed core default set:
   - `file_read`;
   - `file_edit` or patch/edit equivalent;
   - `file_write` only when clearly needed;
   - `bash`;
   - `grep`;
   - `glob`;
   - `todo`;
   - limited `git_status`/`git_diff`;
   - memory read/proposal tools only when memory is enabled.
3. Move long-tail tools behind route/config:
   - GitHub;
   - browser;
   - desktop;
   - remote;
   - team;
   - telemetry;
   - workbench;
   - voice;
   - notebook;
   - install dependencies;
   - share;
   - tool search.
4. Make `ToolExposureReport` the user/debug explanation for why a tool is or is
   not available.
5. Keep hard permission and checkpoint gates independent of exposure.

Acceptance criteria:

- Default tool schema token count drops measurably.
- A coding task still has enough tools for normal file edit and validation.
- A non-coding chat turn gets a smaller or no tool set.
- Long-tail tools can still be enabled explicitly.

Suggested validation:

```bash
cargo test -q route_scoped_tools
cargo test -q tool_exposure
cargo test -q tools
cargo test -q permission
bash scripts/file-size-report.sh
```

### Phase 4: Memory UX Simplification

Goal:

Keep advanced memory governance internally, but present memory to users with a
simple operational model.

Work items:

1. Define the user-facing memory model:
   - project memory docs;
   - local/personal memory docs;
   - accepted durable fact index;
   - proposal queue;
   - current-session memory notices.
2. Add a `/memory status` or equivalent panel section that answers:
   - which memory docs loaded;
   - which index loaded;
   - where accepted records live;
   - how many proposals are pending;
   - whether memory use/generate/active are on;
   - whether new memory applies now or next session.
3. Align docs with this model:
   - `MEMORY_PRODUCT_CONTROL_PLAN_2026-06-02.md`;
   - `MEMORY_SYSTEM_SIMPLIFICATION_PLAN_2026-06-02.md`;
   - a short user-facing "How Memory Works" page.
4. Ensure current-session memory changes use a turn-tail note rather than
   changing the stable prefix.
5. Keep auto-persistence review-only by default.

Acceptance criteria:

- A user can answer "where is this memory stored?" from the UI/docs.
- A user can answer "is this memory active now?" from the UI/docs.
- Accepted/proposed/rejected/applied memory states are not conflated.
- Memory tests cover prefix stability plus proposal/apply behavior.

Suggested validation:

```bash
cargo test -q memory
cargo test -q memory_proposals
cargo test -q prompt_context
cargo test -q tui
```

### Phase 5: Config Productization

Goal:

Move stable behavior from scattered environment flags into a documented config
surface while keeping env overrides for developer experiments.

Work items:

1. Define config sections:
   - `[provider]` or `[[providers]]`;
   - `[runtime]`;
   - `[tools]`;
   - `[permissions]`;
   - `[memory]`;
   - `[eval]`;
   - `[ui]`;
   - `[plugins]` or MCP config import.
2. Add effective-config reporting:
   - source: default, project config, user config, env override, CLI flag;
   - redacted secrets;
   - warnings for deprecated env-only settings.
3. Start with provider slow-tail settings:
   - reconnect attempts;
   - reconnect backoff;
   - timeout;
   - slow warning threshold;
   - provider family overrides.
4. Move tool exposure policy into config:
   - default tools;
   - route-scoped tools;
   - explicitly disabled tools.
5. Move memory policy into config:
   - use;
   - generate;
   - active memory;
   - auto write policy;
   - review requirement.

Acceptance criteria:

- `/doctor` reports effective provider/tool/memory config.
- Existing env variables still work but are described as overrides.
- Config changes that bust cache are explicit and traceable.

Suggested validation:

```bash
cargo test -q config
cargo test -q provider_protocol
cargo test -q runtime_timeouts
cargo test -q tool_exposure
cargo test -q memory
```

### Phase 6: Daily Baseline As Product Gate

Goal:

Make daily baseline the answer to "can this software normally run today?"

Work items:

1. Define a stable daily suite:
   - simple file edit;
   - stale edit repair;
   - multi-file refactor;
   - command validation;
   - provider slow/non-streaming path;
   - TUI path;
   - memory recall/proposal path;
   - permission denial path.
2. Freeze report fields:
   - pass/fail;
   - failure owner;
   - provider retries;
   - provider timeouts;
   - slow warnings;
   - validation proof;
   - tool failures;
   - memory proposal count;
   - cache hit/cached token indicators when available.
3. Make generated benchmark directories manageable:
   - docs should link to a small summary;
   - raw run bundles should be optional or ignored unless explicitly retained.
4. Add a one-command product gate:

```bash
bash scripts/product-daily-gate.sh
```

Acceptance criteria:

- Daily baseline can be rerun without polluting git by default.
- The summary makes weak-model failures distinct from runtime-flow failures.
- Provider slow-tail metrics are visible in daily summaries.
- TUI/headless path divergence is detectable.

Suggested validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
bash scripts/product-daily-gate.sh
```

### Phase 7: Distribution And First-Run Polish

Goal:

Create a clear product path from "new checkout/install" to "successful first
coding task".

Work items:

1. Add or refresh setup docs:
   - install/build command;
   - provider key setup;
   - launch CLI/TUI;
   - launch desktop;
   - run first coding task;
   - where config/memory/session files live.
2. Add a setup/doctor flow:
   - provider key present;
   - provider auth check;
   - toolchain check;
   - workspace write check;
   - memory store check;
   - daily baseline availability.
3. Make packaged desktop and TUI paths share the same runtime smoke test.
4. Keep setup docs short enough for real use.

Acceptance criteria:

- A new user can run one command to diagnose setup.
- Product docs explain the difference between CLI, TUI, and desktop.
- The first-run path is part of daily/product gate coverage.

## Concrete Next Sprint Recommendation

The best next sprint is Phase 1 plus a narrow slice of Phase 2:

1. Create a runtime facade map:
   - list all current frontend/runtime entrypoints;
   - list which state each currently owns;
   - mark target owner after consolidation.
2. Move provider request lifecycle state behind the shared runtime event surface.
3. Add cache-stability assertions for:
   - provider slow warning;
   - provider retry;
   - memory proposal creation;
   - plan mode toggle.
4. Add a small product-facing doc:
   - "What changes the prefix cache?"
   - "What is turn-tail context?"
5. Run:

```bash
cargo fmt --check
cargo check -q
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q runtime_timeouts
cargo test -q tui
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
```

Why start there:

- It builds directly on the provider slow-tail work just completed.
- It reduces future TUI/desktop divergence.
- It turns cache stability from an architectural claim into a regression-tested
  contract.
- It does not add broad new feature surface.

## Non-Goals

- Do not remove validation proof, permission gates, checkpoint behavior, or
  honest failure ownership.
- Do not make memory auto-write broadly permissive just to feel simpler.
- Do not collapse all tools into one generic shell/file interface.
- Do not chase Reasonix feature parity blindly; use Reasonix as a product-shape
  reference, not as the final architecture.
- Do not make generated benchmark run bundles default committed artifacts.

## Done Definition For This Product-Hardening Stage

This stage is done when:

- CLI/TUI/desktop/eval share one runtime event and state facade.
- The stable prefix contract has direct tests and visible diagnostics.
- Default tool exposure is smaller, route-scoped, and explainable.
- Memory has a simple user-facing model while retaining review/safety gates.
- Provider slow-tail behavior is configured, visible, and represented in daily
  baseline reports.
- Daily baseline can be run repeatedly without dirtying the repo by default.
- The first-run product path is documented and testable.

At that point, Priority Agent will still be deeper than Reasonix, but it should
feel closer to Reasonix in the ways that matter for product maturity: predictable
startup, stable cache behavior, narrow default tools, clear memory behavior, and
one runtime path across surfaces.

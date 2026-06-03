# Priority Agent vs Reasonix Gap Analysis

Date: 2026-06-03

## Executive Summary

Priority Agent has reached a stage where its internal mechanisms — validation
proof, permission gates, provider protocol normalization, memory governance,
and runtime traces — are deeper than Reasonix in several important dimensions.

The gap is not mainly missing features. The gap is product shape:

- Reasonix feels smaller because its control boundaries are cleaner.
- Reasonix is easier to explain because memory, tools, config, and provider
  behavior have plain user-facing models.
- Reasonix is more cache-oriented because it treats the system prompt prefix,
  tool schemas, and loaded memory as a stable product contract.
- Priority Agent currently has stronger internal mechanisms, but they are still
  spread across many runtime, TUI, desktop, memory, provider, and tool modules.

This document defines the concrete gaps and a prioritized plan to close them.

## Reference Snapshot

Reasonix source reviewed from:

- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/REASONIX.md`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/control/controller.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/agent/agent.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/memory/memory.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/provider/provider.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/tool/tool.go`
- `/Users/georgexu/Downloads/DeepSeek-Reasonix-main/internal/config/config.go`

Priority Agent source reviewed from:

- `src/engine/runtime_facade.rs`
- `src/engine/cache_stability.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/services/api/provider_protocol.rs`
- `src/memory/mod.rs`
- `src/tools/mod.rs`
- `src/tui/app.rs`
- `src/tui/screens/main_screen.rs`

## What Priority Agent Does Better

### 1. Validation Proof And Closeout

Priority Agent has a deeper validation and closeout system:

- `VerificationCompleted` trace events with check/tests/review status
- `AcceptanceReviewCompleted` with confidence and criteria tracking
- `FinalCloseoutPrepared` with proof status and residual risk assessment
- `CompletionContractEvaluated` with terminal status and verification proof

Reasonix has a simpler completion model. Priority Agent's approach catches
more edge cases and provides better evidence for why a turn succeeded or failed.

### 2. Provider Protocol Normalization

Priority Agent handles provider-specific quirks more thoroughly:

- `ProviderCapabilities` with per-family flags (supports_streaming_tool_calls,
  requires_nonstreaming_tool_calls, requires_tool_result_adjacency)
- `ProviderMessageNormalization` with system message merging and tool-call
  pair repair
- `ProviderToolCallRepairApplied` with schema flattening and argument repair

Reasonix has a simpler provider model. Priority Agent's approach allows it to
work with MiniMax, Kimi, and other non-standard providers that Reasonix cannot
handle correctly.

### 3. Provider Slow-Tail Telemetry

Priority Agent has comprehensive provider slow-tail handling:

- `ProviderLatencyProfile` with per-family timeout and slow-warning thresholds
- `ProviderRequestLifecycle` with phase tracking (started, retrying, slow_warning,
  completed, timeout, cancelled)
- TUI status bar shows provider wait state, elapsed time, and slow-path warnings
- Daily baseline reports provider timeout count, retry count, and slow warning count

Reasonix has simpler timeout handling. Priority Agent's approach gives users
visibility into why a request is slow and what they can do about it.

### 4. Memory Safety And Quality

Priority Agent has deeper memory governance:

- `MemorySafetyScan` with unsafe content detection
- `MemoryQualityAssessment` with quality scoring and threshold-based decisions
- `MemoryProposal` with review queue and explicit accept/reject flows
- `MemoryWriteDecision` with factors (novelty, durability, specificity, actionability)

Reasonix has a simpler memory model. Priority Agent's approach prevents
low-quality or unsafe memories from polluting the knowledge base.

### 5. Runtime Traces And Daily Baseline

Priority Agent has more comprehensive observability:

- `TraceEvent` with 60+ event types covering the full agent lifecycle
- `RuntimeDietReport` with prompt tokens, tool schema tokens, and context budget
- `CacheStabilitySnapshot` with prefix fingerprint and dynamic zone tracking
- Daily baseline with provider timeout/retry/slow-warning metrics

Reasonix has simpler logging. Priority Agent's approach allows for more
precise debugging and regression detection.

## What Reasonix Does Better

### 1. Controller Boundary Is Cleaner

Reasonix centers product behavior behind one transport-agnostic controller:

```go
// reasonix/internal/control/controller.go
type Controller struct {
    runner   agent.Runner
    executor *agent.Agent
    sink     event.Sink  // all frontends consume event.Sink
    policy   permission.Policy
    mem      *memory.Set
    reg      *tool.Registry
    hooks    *hook.Runner
    jobs     *jobs.Manager
    cp       *checkpoint.Store
    ...
}
```

The chat TUI, HTTP/SSE serve path, and desktop app call into the same control
layer. The agent loop does not need to know the details of each frontend.

Priority Agent has `StreamingQueryEngine` and `RuntimeFacadeState`, but
practical ownership is still spread across:

- `src/engine/query_engine.rs`
- `src/engine/streaming.rs`
- `src/engine/conversation_loop/*`
- `src/tui/app.rs`
- `apps/desktop/src-tauri/src/*`

**Problem:**

- TUI/desktop runtime state can still know too much about provider, memory,
  session, and trace details.
- Product behavior can be fixed in one frontend path but not automatically land
  in another.
- Test coverage has to chase multiple entrypoints instead of one product facade.

**Target:**

- A single runtime facade should own user turn submission, cancellation,
  runtime diagnostics, session persistence, provider status, memory status,
  approvals, and closeout state.
- CLI/TUI/desktop should render runtime events and submit user actions, not
  duplicate runtime policy.

### 2. Cache-Stable Prefix Is A First-Class Product Contract

Reasonix treats the base prompt, tool schemas, and loaded memory as a stable
prefix. Dynamic data is pushed into the current turn tail instead of mutating the
prefix mid-session.

```go
// reasonix/internal/control/compose.go
func (c *Controller) Compose(turn string) []provider.Message {
    // system prompt + tools + memory index = stable prefix
    // turn-specific context = dynamic tail
}
```

Priority Agent has made progress:

- prompt context has stable-prefix reporting;
- dynamic retrieval is separated into context zones;
- provider slow-tail telemetry now uses runtime diagnostics instead of prompt
  rules;
- memory proposals are review-oriented by default.

**Remaining risk:**

- Some context assembly, memory recall, skill/tool exposure, and route policy
  paths still need explicit cache-stability assertions.
- Tool schema order and enabled tool set must be stable for equivalent routes.
- Runtime diagnostics must never leak back into the stable prefix.

**Target:**

- A user-facing invariant: "The session prefix is fixed after turn bootstrap;
  dynamic hints and newly saved memory apply through turn-tail notes or next
  session reload."

### 3. Memory UX Is Simpler

Reasonix memory is easy to explain:

- hierarchical memory docs;
- a per-project memory store;
- one fact per Markdown file;
- `MEMORY.md` as an index;
- `remember` and `forget` as model-visible tools;
- saved index loads into the prefix on the next session;
- current-session changes apply through a queued turn-tail note.

```go
// reasonix/internal/memory/memory.go
type Set struct {
    Docs    []Source // REASONIX.md / AGENTS.md, ascending precedence
    Store   Store    // auto-memory store (may be a zero/disabled Store)
    Index   string   // MEMORY.md contents at load time
    CWD     string   // project working dir used for discovery
    UserDir string   // user config root (may be "")
}
```

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

**Problem:**

- The internal design is strong, but the product story is hard to explain.
- It is not yet obvious to a normal user which memory files are loaded, when a
  memory applies, and whether a newly proposed memory is active now or only
  after review.

**Target:**

- Present memory as three simple layers:
  - Stable memory prefix: project/user memory docs plus an index of accepted
    durable facts.
  - Turn-tail memory updates: notes about memory changes made during this
    session.
  - On-demand memory reads: when the index says a specific fact is relevant,
    the agent reads the source file before relying on it.

The existing safety/proposal machinery can remain underneath, but the user
model should be as simple as Reasonix.

### 4. Configuration Is More Productized

Reasonix exposes provider selection, tools, permissions, sandbox, plugins,
skills, LSP, statusline, and proxy behavior in one TOML-driven product surface.

```toml
# reasonix.example.toml
[providers]
[[providers.entries]]
kind = "deepseek"
model = "deepseek-reasoner"
api_key_env = "DEEPSEEK_API_KEY"

[tools]
enabled = ["bash", "file_read", "file_edit", ...]

[permissions]
mode = "default"

[plugins]
enabled = true
```

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

**Problem:**

- Environment variables are useful for experiments, but they are not enough for
  a commercial-grade product surface.

**Target:**

- Keep env overrides for developer/debug usage.
- Add a stable project/user config layer for product behavior.
- `/config` or `/doctor` should explain the effective configuration and where
  each value came from.

### 5. Tool Parallelization

Reasonix's `Tool.ReadOnly()` interface lets read-only tools run in parallel:

```go
// reasonix/internal/tool/tool.go
type Tool interface {
    Name() string
    Description() string
    Schema() json.RawMessage
    Execute(ctx context.Context, args json.RawMessage) (string, error)
    ReadOnly() bool  // true = can run in parallel
}
```

The agent parallelises a batch of tool calls only when every call in the batch
is ReadOnly; mixed batches stay sequential so write/read ordering is preserved.

Priority Agent has `tool_call_is_read_only` and `tool_call_is_concurrency_safe`,
but does not actually parallelize execution.

**Target:**

- Add read-only tool parallelization in `tool_execution_controller`.
- Keep write tools sequential to preserve ordering.

### 6. Checkpoint/Rewind Is More Complete

Reasonix has a full checkpoint system:

```go
// reasonix/internal/control/controller.go
type Controller struct {
    cp      *checkpoint.Store
    cpRoot  string
    cpTurn  int
    cpBound map[int]int  // turn -> message index boundary
}
```

Checkpoints are per-session and track the conversation boundary for each turn.
This allows precise rewind to any previous turn without losing context.

Priority Agent has `RewindTool` but does not track turn-level checkpoint
boundaries.

**Target:**

- Add turn counter and checkpoint boundary tracking to `RuntimeFacadeState`.
- Allow precise rewind to any previous turn.

### 7. Hook System Is User-Configurable

Reasonix's hooks are configured in TOML:

```toml
# reasonix.example.toml
[hooks]
pre_tool = ["./scripts/lint-check.sh"]
post_tool = ["./scripts/format.sh"]
```

Priority Agent's `hooks.rs` exists but is not exposed to user configuration.

**Target:**

- Add hooks configuration section to `priority-agent.toml`.
- Allow users to configure pre-tool and post-tool hooks.

## Gap Summary Table

| Dimension                  | Priority Agent | Reasonix | Gap   |
|----------------------------|----------------|----------|-------|
| Validation proof           | Deep           | Simple   | Better|
| Provider protocol          | Deep           | Simple   | Better|
| Provider slow-tail         | Deep           | Simple   | Better|
| Memory safety              | Deep           | Simple   | Better|
| Runtime traces             | Deep           | Simple   | Better|
| Controller boundary        | Spread         | Clean    | Gap   |
| Cache-stable prefix        | Good           | Excellent| Gap   |
| Memory UX                  | Complex        | Simple   | Gap   |
| Configuration              | Env vars       | TOML     | Gap   |
| Tool parallelization       | None           | Yes      | Gap   |
| Checkpoint/rewind          | Basic          | Full     | Gap   |
| Hook system                | Internal       | User     | Gap   |

## Prioritized Recommendations

### High Priority (Next Sprint)

#### 1. Complete Runtime Facade Consolidation

Move `provider_request_state` completely into `RuntimeFacadeState`. TUI and
desktop should only render facade events, not directly manipulate provider
lifecycle state.

**Files to change:**

- `src/engine/runtime_facade.rs` — add turn submission, cancellation, approval
- `src/tui/app.rs` — remove direct provider_request manipulation
- `apps/desktop/src-tauri/src/lib.rs` — use facade for all runtime state

**Acceptance:**

- TUI and desktop use the same provider request lifecycle events.
- Tool approval behavior has one owner.
- Session persistence path is not duplicated.

#### 2. Simplify Memory Status Display

Update `/memory status` to show three clear layers:

```
Memory Status

Stable Prefix:
  - project memory: 3 files, 1.2k chars
  - user memory: 1 file, 400 chars
  - accepted facts: 12 records

Turn-Tail Updates:
  - proposals pending: 2
  - proposals accepted this session: 1

On-Demand Reads:
  - retrieval policy: balanced
  - active memory: off

Controls:
  - use: on
  - generate: on
  - recall: balanced
  - write-policy: review_only
```

**Files to change:**

- `src/tui/app/slash_commands.rs` — update `/memory status` handler
- `src/memory/manager.rs` — add summary methods for three-layer display

**Acceptance:**

- A user can answer "where is this memory stored?" from the UI.
- A user can answer "is this memory active now?" from the UI.
- Accepted/proposed/rejected/applied memory states are not conflated.

#### 3. Enhance /doctor With Tool Availability

Add tool availability check to `/doctor`:

```
Tool Availability:
  - registered: 45
  - available: 38
  - hidden by route: 5
  - hidden by permission: 2
  - unavailable: 0
```

**Files to change:**

- `src/tui/slash_handler/agents.rs` — add tool availability section

**Acceptance:**

- `/doctor` shows which tools are registered but not available.
- `/doctor` shows why tools are hidden (route, permission, provider).

### Medium Priority

#### 4. Add Read-Only Tool Parallelization

Parallelize read-only tool execution in `tool_execution_controller`:

**Files to change:**

- `src/engine/conversation_loop/tool_execution_controller.rs` — add
  parallel execution for read-only tools
- `src/tools/mod.rs` — add `ReadOnly` trait method

**Acceptance:**

- Read-only tools (file_read, grep, glob) run in parallel.
- Write tools (file_edit, bash) run sequentially.
- Mixed batches stay sequential.

#### 5. Add Checkpoint Boundary Tracking

Add turn counter and checkpoint boundary tracking to `RuntimeFacadeState`:

**Files to change:**

- `src/engine/runtime_facade.rs` — add turn counter and checkpoint boundaries
- `src/tools/rewind_tool.rs` — use boundaries for precise rewind

**Acceptance:**

- Each turn has a monotonic counter.
- Rewind can target any previous turn by counter.
- Checkpoint boundaries are persisted in session.

#### 6. Add Configuration File Support

Add `priority-agent.toml` configuration file support:

**Files to change:**

- `src/services/config.rs` — add TOML config loading
- `src/tui/slash_handler/config.rs` — update `/config` to show file values

**Acceptance:**

- `priority-agent.toml` is loaded from project root.
- Environment variables override config file values.
- `/config effective` shows source (default/config/env) for each value.

### Low Priority

#### 7. Add User-Configurable Hooks

Add hooks configuration section:

**Files to change:**

- `src/services/config.rs` — add hooks config section
- `src/engine/hooks.rs` — load hooks from config

**Acceptance:**

- Users can configure pre-tool and post-tool hooks in `priority-agent.toml`.
- Hooks run before/after tool execution.

## Done Definition

This gap analysis is addressed when:

- CLI/TUI/desktop/eval share one runtime event and state facade.
- Memory has a simple three-layer user-facing model while retaining
  review/safety gates.
- Default tool exposure is smaller, route-scoped, and explainable.
- Provider slow-tail behavior is configured, visible, and represented in
  daily baseline reports.
- `/doctor` reports effective provider/tool/memory config and tool availability.
- The first-run product path is documented and testable.

At that point, Priority Agent will still be deeper than Reasonix in validation,
provider protocol, and memory safety, but it should feel closer to Reasonix in
the ways that matter for product maturity: predictable startup, stable cache
behavior, narrow default tools, clear memory behavior, and one runtime path
across surfaces.

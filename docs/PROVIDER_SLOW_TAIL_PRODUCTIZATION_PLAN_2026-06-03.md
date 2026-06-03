# Provider Slow-Tail Productization Plan

Date: 2026-06-03

Status: proposed next-phase engineering plan

## Summary

Provider slow tail is now a product problem, not just a timeout constant.

The 2026-06-03 daily gate showed three MiniMax-M3 failures caused by
non-streaming chat timing out after 180s. This is expected for MiniMax-style
tool-call turns because provider capabilities currently mark MiniMax as:

- `requires_nonstreaming_tool_calls = true`
- `supports_streaming_tool_calls = false`
- `requires_tool_result_adjacency = true`

The runtime already has useful foundations:

- provider-bound retry/reconnect at `src/services/api/retry.rs`;
- request timeout via `PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS`;
- stream idle timeout via `PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS`;
- turn timeout via `PRIORITY_AGENT_TURN_TIMEOUT_SECS`;
- provider protocol facts in `src/services/api/provider_protocol.rs`;
- runtime diagnostic events for API request start and provider family;
- fallback from streaming open failures to non-streaming requests;
- live/daily eval reporting that can classify provider timeout as
  environment/provider failure.

The gap is that this is still technical behavior. Users need visible progress,
clear cancellation, safe fallback, and predictable daily-baseline accounting.

## Product Goal

Make slow providers feel controlled instead of stuck.

The user should always be able to answer:

1. Is the model thinking, waiting on provider, running tools, retrying, or
   timed out?
2. How long has this provider request been running?
3. Is this provider/model known to be slow for tool calls?
4. Can I cancel safely?
5. Can the runtime recover with a faster provider or smaller request without
   corrupting tool state?
6. Did daily eval fail because the agent made a bad decision or because the
   provider timed out?

## Non-Goals

- Do not weaken validation, permissions, checkpoints, or closeout proof to
  hide provider latency.
- Do not blindly increase every timeout. Longer waits may improve green rate
  while making the product feel worse.
- Do not retry after local side effects. Provider reconnect may only retry an
  outbound LLM request before a usable response is accepted.
- Do not add provider-specific prompt hacks for one model's slow behavior.
- Do not enable speculative multi-provider execution by default.

## Current State

### Request Boundaries

`src/engine/conversation_loop/session_processor.rs` wraps non-streaming chat
with `llm_request_timeout()`. The current default is 180s, clamped between 30s
and 600s.

Streaming open is also wrapped by `llm_request_timeout()`. Once a stream is
open, each chunk waits up to `stream_chunk_idle_timeout()`, currently defaulted
to 120s.

`src/engine/streaming.rs` wraps the whole turn with
`turn_execution_timeout()`, currently defaulted to 1800s.

### Provider Protocol Facts

`src/services/api/provider_protocol.rs` already distinguishes provider
capabilities. MiniMax is correctly modeled as a provider that supports tool
calls but requires non-streaming tool-call requests.

`src/engine/conversation_loop/api_request_controller.rs` already detects this
case and emits a runtime diagnostic with:

- provider family;
- tool count;
- streaming vs non-streaming request path;
- `nonstreaming_tool_request`.

### Retry Layer

`src/services/api/retry.rs` retries retryable transport/provider errors with
exponential backoff. This is the right boundary because tool execution is
outside this layer.

The current retry layer does not yet expose structured retry events, elapsed
time, attempt count, or timeout/failure kind to the runtime UI and daily
reports.

### Eval Evidence

`docs/DAILY_GATE_TEST_REPORT_2026-06-03.md` recorded:

- three failures from `non-streaming chat timed out after 180s`;
- the root cause as MiniMax-M3 high latency on non-streaming tool requests;
- possible mitigations including longer timeout, faster model, smaller prompt,
  and streaming tool support.

## Design Direction

Treat provider calls as a state machine with a user-visible lifecycle:

```text
queued -> request_started -> first_byte_or_nonstream_wait
       -> retrying | slow_warning | fallback_considered
       -> completed | timeout | cancelled | provider_failed
```

For streaming-capable providers, "first byte" and idle progress matter.

For MiniMax-style non-streaming tool requests, the runtime cannot show token
chunks, so it must show elapsed wait, known slow-path reason, retry attempts,
and safe cancellation.

## Phase 1: Provider Latency Profile And Trace Events

Goal: make provider slow tail measurable before changing behavior.

Tasks:

1. Add `ProviderLatencyProfile` near provider protocol/config code.
2. Derive defaults from provider family and request shape:
   - standard streaming text request;
   - streaming tool-call request;
   - non-streaming tool-call request;
   - fallback non-streaming request.
3. Include configured timeout, slow-warning threshold, provider family, model,
   request path, message count, tool count, estimated prompt tokens when
   available.
4. Add trace/runtime diagnostic events:
   - `provider.request.started`
   - `provider.request.retrying`
   - `provider.request.slow_warning`
   - `provider.request.completed`
   - `provider.request.timeout`
   - `provider.request.cancelled`
5. Keep retry at provider boundary. The retry layer should return structured
   attempt metadata, not make runtime policy decisions.

Candidate files:

- `src/services/api/provider_protocol.rs`
- `src/services/api/retry.rs`
- `src/engine/conversation_loop/runtime_timeouts.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/trace.rs`
- `src/engine/evidence_ledger.rs`
- `scripts/live_eval_report_parser.py`

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q provider_protocol -- --test-threads=1
cargo test -q retry -- --test-threads=1
cargo test -q runtime_timeouts -- --test-threads=1
cargo test -q trace -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

## Phase 2: User-Visible Slow State In TUI And Desktop

Goal: the app should never look frozen during a slow provider request.

Tasks:

1. Convert provider diagnostics into visible run state:
   - waiting for provider;
   - non-streaming tool request;
   - retrying provider request;
   - slow provider warning;
   - fallback attempt;
   - timed out.
2. Display elapsed provider wait time separately from total turn time.
3. Show why streaming is unavailable when the request path is non-streaming:
   "MiniMax tool-call requests use non-streaming mode for protocol
   compatibility."
4. Add a safe cancel action that aborts the active turn/task and records
   `provider.request.cancelled`.
5. Keep partial transcript state honest. A cancelled provider request should
   not create fake assistant/tool messages.

Candidate files:

- `src/tui/app.rs`
- `src/tui/tool_view.rs`
- `src/tui/screens/main_screen.rs`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/diagnostics.rs`

Acceptance:

```bash
cargo fmt --check
cargo test -q tui -- --test-threads=1
(cd apps/desktop/src-tauri && cargo fmt --check && cargo test -q)
scripts/runtime-entrypoint-smoke.sh --tui --timeout 5
scripts/runtime-entrypoint-smoke.sh --desktop-quick
```

## Phase 3: Timeout Policy By Route And Provider

Goal: use explicit policy instead of one global timeout.

Tasks:

1. Replace the single default 180s request timeout with a profile-driven
   timeout:
   - fast text/no-tool turn: shorter timeout;
   - streaming request open: medium timeout;
   - non-streaming tool-call request: longer but visible timeout;
   - fallback request without tools: shorter timeout;
   - summarization/compaction helper request: bounded timeout.
2. Keep environment overrides, but document them as escape hatches.
3. Emit slow warning before timeout, for example after 45-60s on standard
   requests and 90-120s on known slow non-streaming tool calls.
4. Tie timeout policy to `ResourcePolicySelected`/route latency where
   practical:
   - direct answer should not wait like deep coding;
   - high-risk coding may wait longer, but must remain cancellable.
5. Distinguish timeout owner:
   - provider timeout;
   - stream idle timeout;
   - full turn timeout;
   - validation timeout.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q runtime_timeouts -- --test-threads=1
cargo test -q conversation_loop -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

## Phase 4: Safe Fallback And Recovery Policy

Goal: give the runtime a controlled path after provider slow-tail failure.

Tasks:

1. Add a `ProviderFailureKind` classification:
   - timeout;
   - stream idle;
   - transient transport;
   - rate limit/quota;
   - auth/config;
   - provider protocol/schema;
   - context too long;
   - cancelled.
2. For timeout/slow-tail only, allow one bounded recovery path:
   - retry same provider only if no response was accepted;
   - optionally retry with smaller request after compaction/snip;
   - optionally retry fallback provider/model if configured and capability
     compatible.
3. Never fallback across a provider boundary after a tool result has already
   been accepted into provider-visible history unless the runtime can preserve
   valid tool-call pairing.
4. If fallback is skipped, produce a concise user-facing explanation with next
   actions:
   - wait/retry;
   - switch provider;
   - reduce context;
   - run in prepare/plan-only mode;
   - increase timeout explicitly.
5. Record fallback outcome in trace and daily reports.

Candidate files:

- `src/engine/error_classifier.rs` or new provider failure module;
- `src/engine/conversation_loop/turn_api_failure_controller.rs`;
- `src/engine/conversation_loop/session_processor.rs`;
- `src/engine/recovery_plan.rs`;
- `src/services/api/retry.rs`;
- `src/services/api/provider_protocol.rs`.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q api_failure -- --test-threads=1
cargo test -q recovery_plan -- --test-threads=1
cargo test -q provider_protocol -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

## Phase 5: Daily Baseline And Report Integration

Goal: provider slow tail should not be confused with agent-flow regression.

Tasks:

1. Extend live/daily report parser with provider metrics:
   - provider family/model;
   - request path;
   - timeout kind;
   - retry attempts;
   - time to first chunk;
   - max stream idle gap;
   - non-streaming wait duration;
   - fallback attempted/succeeded/skipped.
2. Add daily summary buckets:
   - `provider_timeout`;
   - `provider_slow_tail_recovered`;
   - `provider_slow_tail_unrecovered`;
   - `provider_protocol_failure`;
   - `agent_flow`.
3. Keep `failure_owner=environment/provider` when the model never produced a
   usable response.
4. Add a provider slow-tail smoke fixture with a mock provider:
   - slow but completes;
   - slow warning emitted;
   - timeout emitted;
   - cancel emitted;
   - retry metadata recorded.
5. Update `docs/DAILY_GATE_TEST_REPORT_2026-06-03.md` or a follow-up daily
   report after the first implementation run.

Candidate files:

- `scripts/live_eval_report_parser.py`
- `scripts/product_daily_summary.py`
- `scripts/live_eval_quality_status.py`
- `evalsets/live_tasks/core-provider-roundtrip.yaml`
- `docs/DAILY_GATE_TEST_REPORT_2026-06-03.md`

Acceptance:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py scripts/product_daily_summary.py scripts/live_eval_quality_status.py
bash scripts/live-eval-summary-smoke.sh
bash scripts/product-daily-gate.sh --dry-run --layer product
cargo test -q provider_slow_tail -- --test-threads=1
```

## Phase 6: Provider Selection And User Controls

Goal: make provider choice a product control, not an environment accident.

Tasks:

1. Add provider status labels:
   - configured;
   - healthy;
   - slow-tool-call path;
   - quota/auth problem;
   - recommended for coding;
   - recommended for fast direct answers.
2. Add TUI/desktop controls for:
   - current provider/model;
   - fallback provider/model;
   - request timeout profile;
   - cancel active run;
   - retry with faster provider.
3. Keep CLI env vars working for power users.
4. Add a short provider doctor output:
   - configured providers;
   - protocol capabilities;
   - timeout profile;
   - last health check;
   - last timeout/failure.

Acceptance:

```bash
cargo fmt --check
cargo test -q provider -- --test-threads=1
cargo test -q tui -- --test-threads=1
(cd apps/desktop/src-tauri && cargo test -q)
bash scripts/health-check.sh
```

## Implementation Order

Recommended first slice:

1. Add structured provider request lifecycle events and latency profile.
2. Wire those events through the existing runtime diagnostic channel.
3. Add TUI-visible "waiting on provider" and "non-streaming tool request"
   states.
4. Add report parser fields for timeout kind and request path.

Do not start by increasing timeout globally. A longer timeout may be useful for
MiniMax coding tasks, but only after the user can see and cancel the slow path.

## Success Metrics

Daily baseline should report:

- provider timeout count;
- provider timeout by model;
- median/p95 provider request duration;
- non-streaming tool request count;
- slow-warning count;
- cancel count;
- fallback attempted/succeeded/skipped count;
- agent-flow failure count excluding provider-owned timeouts.

Product-level target:

- no run should appear frozen for more than 10 seconds without visible state;
- known slow non-streaming provider requests should emit a warning before
  timeout;
- provider-owned failures should be distinguishable from agent-flow failures;
- cancellation should leave no malformed provider-visible tool history;
- fallback should be opt-in or explicitly configured until enough evidence
  proves it is safe.

## Open Questions

1. Should MiniMax-M3 get a default non-streaming tool-call timeout of 300s, or
   should longer waits remain opt-in until visible slow-state UI lands?
2. Which provider should be the first recommended fallback for coding turns:
   Kimi Code, DeepSeek, GLM, or user-configured OpenAI-compatible?
3. Should fallback provider/model be selected per route, or only after explicit
   user configuration?
4. Should daily gate treat provider timeout as neutral/skipped, failed
   environment, or separate "provider blocked" status?
5. Should desktop expose provider doctor as a persistent status panel or only
   in diagnostics?


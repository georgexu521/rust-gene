# Provider Certification Matrix
Status: Reference

Product-auditable summary of provider-family behavior. Protocol rows are
reasoned about through known contracts and golden tests; LabRun rows require
explicit live evidence.

Last updated: 2026-06-20

---

## Matrix

| Behavior | DeepSeek (OpenAI-compat) | MiniMax | Kimi | Notes |
|----------|--------------------------|---------|------|-------|
| Request field naming | `messages`, `tools` | `messages`, `tools` | `messages`, `tools` | Standard OpenAI shape |
| Max output field | `max_tokens` | `max_tokens` | `max_tokens` | Capped via env |
| Tool-call shape | `tool_calls[]: {id, type:"function", function:{name, arguments}}` | same | same | Deserialized via `ToolCall` |
| Streaming tool calls | ✅ | ❌ (requires non-streaming) | ✅ | MiniMax: synchronous only |
| Non-streaming fallback | N/A | ✅ always when tools present | N/A | See `ProviderCapabilities::for_family` |
| Usage extraction | `usage.prompt_tokens`, `usage.completion_tokens`, `usage.completion_tokens_details.reasoning_tokens` | same (no reasoning tokens) | same | |
| Cached-token extraction | `usage.prompt_tokens_details.cached_tokens` | cached/read not currently mapped | `usage.prompt_tokens_details.cached_tokens` | |
| Cache-write extraction | not currently exposed by adapter | `usage.prompt_tokens_details.cache_write_tokens` plus compatible aliases | not currently exposed by adapter | Recorded as `cache_write_tokens` when provider reports it |
| Token counter | OpenAI GPT-4o/GPT-4.1/reasoning profiles use `tiktoken-rs` | CJK-heavy fallback | CJK-heavy fallback | Provider usage remains source of truth after response |
| Cache-write pricing | provider/model/global env override lanes | provider/model/global env override lanes | provider/model/global env override lanes | `PRIORITY_AGENT_COST_<PROVIDER>_CACHE_WRITE_PER_1K`, model-specific and global fallbacks |
| Reasoning / interleaved | `reasoning_content` in delta | none | `reasoning_content` in delta | DeepSeek emits interleaved; see `ProviderTransformReport` |
| Empty reasoning parts | preserved (or inserted) | N/A | N/A | See golden tests |
| DSML leaked calls | occasionally emits `\n\n`-wrapped function calls | unknown | unknown | Stripped by `tool_call_repair.rs` |
| Timeout classification | auth/rate_limit/protocol/schema/timeout/transport | same | same | See `provider_health_error_category` |
| Retry safety | safe for idempotent tools only | same | same | Non-streaming requests are not retried post-write |
| System message merging | not needed | ✅ merges to single message | not needed | |
| Strict tool schema | env `PRIORITY_AGENT_ENABLE_STRICT_TOOL_SCHEMA` | same | same | Optional per provider |

---

## LabRun Live Certification

LabRun has a stricter product-level certification than protocol-level tool
shape tests. A provider is not certified for autonomous graduate code-writing
until a live run proves all of these in one isolated graduate task:

- model calls `file_write` or `file_edit` for the scoped mutation;
- model calls `bash` for the required validation;
- runtime observes real git changes in the isolated worktree;
- runtime validation passes in that worktree;
- `/lab task worktree review`, `merge`, and `cleanup` complete.

| Provider/model | Professor control plane | Graduate structured JSON | Graduate tool-backed mutation | Runtime verification | Worktree review/merge/cleanup | Evidence |
|----------------|-------------------------|--------------------------|-------------------------------|----------------------|-------------------------------|----------|
| DeepSeek `deepseek-v4-flash` | ✅ live validated | ✅ emitted bindable JSON | ⚠️ generic subagent live path now proves `bash,file_write,file_read` with isolated-worktree file proof; formal Lab graduate remains uncertified and gated | ✅ runtime rejects empty real diff and blocks Lab graduate before subagent reruns | ⏸️ blocked before worktree merge | control-plane: `target/lab-live-validation/20260619-223257`, refreshed with indexed summaries at `target/lab-live-validation/20260620-000209`, provider report at `target/lab-live-validation/20260620-001212/12a-provider-certification.txt`; same-provider compare: `target/lab-live-validation/20260620-121036/13a-provider-compare.txt`, `target/lab-live-validation/20260620-125252/13a-provider-compare.txt`, refreshed generic proof at `target/lab-live-validation/20260620-generic-subagent-product-closeout/evidence/provider-compare.txt`; direct tool probes: `target/lab-live-validation/20260620-125252/12b-provider-tool-diagnostics.txt`; graduate evidence: `target/lab-live-validation/20260619-220330`, `target/lab-live-validation/20260619-220702`, gate: `target/lab-live-validation/20260619-223243`, refreshed gate: `target/lab-live-validation/20260620-121300/15-live-task-run.txt` |
| MiniMax | unverified | unverified | unverified | unverified | unverified | pending |
| Kimi | unverified | unverified | unverified | unverified | unverified | pending |
| OpenAI-compatible custom | unverified | unverified | unverified | unverified | unverified | pending |

Use:

```bash
scripts/lab-live-validation.sh --live-control-plane
scripts/lab-live-validation.sh --live-graduate
```

`--live-control-plane` validates the provider-backed professor/control-plane
path and daemon worker without requiring graduate tool use.
`--live-graduate` is the full certification gate and is expected to fail for
providers that return textual completion claims instead of tool calls.
Known-unsupported rows, currently DeepSeek `deepseek-v4-flash` for graduate
code-writing, are also blocked by the Lab runtime before launching a graduate
subagent unless `PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER=1` is
set for an explicit experimental run.
In live modes, failed validation attempts are also recorded as local `failed`
certification records when the script exits non-zero before writing a success
record. Failed graduate records are visible in `/lab provider`, but do not
certify the provider/model for graduate execution.

Successful live validation records machine-readable local evidence in
`.priority-agent/lab/provider_certifications.jsonl` through:

```bash
pa lab --command "provider record <control-plane|graduate> <passed|failed> <evidence_path> [summary]" --with-provider
```

`/lab provider` displays the latest certification records for the active
provider/model. A latest local `graduate passed` record promotes that
provider/model to `certified` for Lab graduate execution; without that evidence
the static known-unsupported gate still applies.

`/lab provider compare` adds a same-provider diagnostic between the generic
`implementer` subagent and the formal Lab graduate dispatch path. As of
2026-06-20, generic subagent `tools_used` is runtime-observed through the
conversation loop and propagated through `QueryResult` and `AgentResult`; a
textual claim that a tool was used is not counted unless a real tool call
entered the runtime tool execution pipeline.

`/lab provider diagnose-tools` adds direct provider probes for minimal and
runtime tool schemas. DeepSeek `deepseek-v4-flash` currently succeeds with
`tool_choice=auto` for `lab_provider_echo`, `file_write`, and the generic
subagent allowed-tool schema (`file_write,file_edit,bash,diff`), but rejects
`tool_choice=required` and forced named tool calls in thinking mode. That means
DeepSeek is not disqualified at the raw function-calling transport/schema layer;
its remaining LabRun blocker is the full Agent loop completing real
tool-backed work under runtime verification.

---

## Golden Test Coverage

| Provider | Test File | What's Covered |
|----------|-----------|----------------|
| DeepSeek | `provider_protocol.rs` | text-only assistant, reasoning-only, tool calls + empty reasoning, DSML stripping |
| MiniMax | `provider_protocol.rs` | system message merge, non-streaming routing, latency profile |
| Kimi | `provider_protocol.rs` | protocol family detection, standard tool roundtrip |
| All families | `provider_protocol.rs` | provider matrix: valid roundtrips, orphan drops, incomplete downgrades, multi-tool preservation |

Run with:

```bash
cargo test -q provider_protocol    # 19 tests
cargo test -q provider_health       # 3 tests
cargo test -q minimax              # provider-specific
cargo test -q kimi                 # provider-specific
cargo test -q openai_compat        # protocol adapter
```

---

## Provider Status Fields

`GET /api/provider/status` returns `ProviderStatusPage` with per-provider:

- `provider_id`, `model_id` — identity
- `connection_source` — `env`, `runtime`, or `config`
- `protocol_family` — `openai_compatible`, `minimax`, `kimi`, `anthropic_like`
- `supports_streaming_tool_calls` / `requires_nonstreaming` — capability flags
- `context_limit` / `output_limit` — model limits from `ModelContextProfile`
- `request_timeout_secs` / `stream_idle_timeout_secs` — effective timeout config
- `timeout_effective` — top-level timeout config/source used by the API runtime
- `latest_health_status` — `ok`, `failed`, or `null` (unverified)
- `latest_timeout_category` — `auth`, `rate_limit`, `protocol`, `schema`, `timeout`, `transport`
- `capability_summary` — human-readable one-liner

"Unverified" means no health check has run yet for this provider/model pair.
"Configured but unavailable" means env/config exists but preflight would fail.

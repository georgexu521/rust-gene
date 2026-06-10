# Provider Certification Matrix
Status: Reference

Product-auditable summary of provider-family behavior.  Each family is
reasoned about through its known contracts rather than "tried and seen".

Last updated: 2026-06-07

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
| Cached-token extraction | `usage.prompt_tokens_details.cached_tokens` | none | `usage.prompt_tokens_details.cached_tokens` | |
| Reasoning / interleaved | `reasoning_content` in delta | none | `reasoning_content` in delta | DeepSeek emits interleaved; see `ProviderTransformReport` |
| Empty reasoning parts | preserved (or inserted) | N/A | N/A | See golden tests |
| DSML leaked calls | occasionally emits `\n\n`-wrapped function calls | unknown | unknown | Stripped by `tool_call_repair.rs` |
| Timeout classification | auth/rate_limit/protocol/schema/timeout/transport | same | same | See `provider_health_error_category` |
| Retry safety | safe for idempotent tools only | same | same | Non-streaming requests are not retried post-write |
| System message merging | not needed | ✅ merges to single message | not needed | |
| Strict tool schema | env `PRIORITY_AGENT_ENABLE_STRICT_TOOL_SCHEMA` | same | same | Optional per provider |

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

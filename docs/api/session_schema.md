# Session API DTO Schema

Stable data-transfer objects shared by TUI, desktop, and the experimental API
server.  These DTOs are the canonical vocabulary for session state — frontends
consume them directly rather than re-interpreting raw trace internals.

Status: P2 — DTOs defined, experimental API feature-gated, desktop integration
in progress. Desktop frontend code should import session/provider/tool-output
DTOs from `apps/desktop/src/runtime/desktopApi.ts` rather than redefining raw
backend payload shapes inside components.

---

## Session Info

```json
{
  "id": "string",
  "title": "string",
  "model": "string",
  "parent_session_id": "string | null",
  "created_at": "datetime",
  "updated_at": "datetime",
  "total_input_tokens": "i64",
  "total_output_tokens": "i64"
}
```

`SessionStore::get_session(id)` → `Option<SessionRecord>`

---

## Session Parts Cursor

Paginated projection of typed session parts (text, reasoning, tool calls,
closeout, revert, compaction, permissions).

```json
{
  "session_id": "string",
  "parts": [
    {
      "part_id": "string (stable: text_{seq}, tool_{call_id}, closeout_{seq}, ...)",
      "part_index": "i64",
      "kind": "assistant_text | reasoning | tool | shell | permission | compaction | closeout | revert",
      "tool_call_id": "string | null",
      "tool_name": "string | null",
      "status": "pending | running | completed | failed | timed_out | cancelled",
      "timeline_label": "string",
      "diff_summary": "string | null",
      "payload": "<SessionPart as JSON>",
      "projected_to_seq": "i64",
      "updated_at": "datetime"
    }
  ],
  "cursor": {
    "after_part_index": "i64 | null",
    "has_more": "bool"
  }
}
```

API:
- `SessionStore::get_session_parts_after(session_id, after_part_index, limit)` → cursor read
- `SessionStore::get_session_parts(session_id)` → full read (small sessions)

---

## Session Reverts

Durable revert/unrevert metadata for user-facing history, diagnostics, and
desktop dim/trim projection.

```json
{
  "id": "i64",
  "session_id": "string",
  "operation": "revert | unrevert",
  "status": "completed | partial | failed",
  "message_id": "string | null",
  "target_part_id": "string | null",
  "part_ids": ["string"],
  "checkpoint_ids": ["string"],
  "snapshot_checkpoint_id": "string | null",
  "paths": ["string"],
  "restored_files": ["string"],
  "removed_files": ["string"],
  "errors": ["string"],
  "diff_summary": "string | null",
  "unrevert_possible": "bool",
  "unreverted": "bool",
  "payload": "object",
  "created_at": "datetime"
}
```

API:
- `SessionStore::record_session_revert(insert)` → durable revert row
- `SessionStore::list_session_reverts(session_id, limit)` → latest rows
- Desktop `list_session_reverts(sessionId, limit)` → same DTO for frontend use

---

## Session Events Cursor

Raw event stream for audit/debug.  Not intended for routine UI rendering;
prefer session parts for typical display.

```json
{
  "session_id": "string",
  "events": [
    {
      "id": "i64",
      "seq": "i64 (monotonic per session)",
      "event_type": "string",
      "timestamp_ms": "i64",
      "payload": "string (JSON)"
    }
  ],
  "cursor": {
    "after_seq": "i64 | null",
    "has_more": "bool"
  }
}
```

API:
- `SessionStore::get_session_events_after(session_id, after_seq)` → cursor read
- `session_parts::query_session_events_page(session_id, after_seq, limit)` → paged

---

## Tool Output Index / Page

```json
{
  "session_id": "string",
  "outputs": [
    {
      "id": "string",
      "uri": "tool-output://{id}",
      "session_id": "string",
      "tool_call_id": "string",
      "tool_name": "string",
      "mime": "string",
      "original_bytes": "u64",
      "created_at_ms": "u64"
    }
  ]
}
```

Page read:
```json
{
  "content": "string (UTF-8 safe page)",
  "offset": "u64",
  "limit": "u64",
  "total_bytes": "u64",
  "has_more": "bool"
}
```

API:
- `ToolOutputStore::list_for_session(session_id)` → index
- `ToolOutputStore::read_page(session_id, id_or_uri, offset, limit)` → page

---

## Permission Request / Answer

```json
{
  "request_id": "string",
  "tool_name": "string",
  "arguments": "object",
  "prompt": "string",
  "metadata": {
    "risk_level": "low | medium | high",
    "command_category": "string | null",
    "shell_view": {
      "primary_command": "string",
      "mutation_family": "string",
      "detected_paths": ["string"],
      "write_targets": ["string"],
      "recommended_tool": "string | null"
    }
  }
}
```

Answer:
```json
{
  "request_id": "string",
  "decision": "allow | deny | ask",
  "scope": "once | session | project | global",
  "rule_added": "bool",
  "expires_at": "datetime | null"
}
```

API:
- `PermissionContext::explain_decision(tool_name, params)` → `ExplainableDecision`
- `ExplainableDecision.rule_views` → `Vec<PermissionRuleView>`

---

## Provider Status Page

The `GET /api/provider/status` route returns a `ProviderStatusPage` DTO
(not a single runtime profile object).  Each entry is a `ProviderProductStatus`
with stable product fields.

```json
{
  "statuses": [
    {
      "provider_id": "string",
      "label": "string",
      "model_id": "string",
      "model_display_name": "string",
      "connection_source": "env | config | runtime | custom",
      "configured": "bool",
      "active": "bool",
      "disabled": "bool",
      "base_url_host": "string (host only, no credentials)",
      "protocol_family": "openai_compatible | minimax | kimi | anthropic_like",
      "supports_streaming_tool_calls": "bool",
      "requires_nonstreaming": "bool",
      "context_limit": "u64 | null",
      "output_limit": "u64 | null",
      "auto_compact_threshold": "u64 | null",
      "token_counter": "string",
      "cache_accounting": "string",
      "configured_max_output": "u64 | null",
      "cost_input_per_1m": "f64 | null",
      "cost_output_per_1m": "f64 | null",
      "cost_cache_read_per_1m": "f64 | null",
      "cost_cache_write_per_1m": "f64 | null",
      "tool_schema_transform": "string",
      "prompt_delta": "string",
      "latest_health_status": "ok | failed | null",
      "latest_timeout_category": "string | null",
      "last_request_latency_ms": "u64 | null",
      "request_timeout_secs": "u64",
      "stream_idle_timeout_secs": "u64",
      "capability_summary": "string"
    }
  ],
  "record_count": "usize",
  "timeout_effective": {
    "request_secs": "u64",
    "stream_idle_secs": "u64",
    "slow_warning_secs": "u64",
    "max_retry_attempts": "u32",
    "source": "default | env"
  }
}
```

API:
- `GET /api/provider/status` → `ProviderStatusPage`
- `GET /api/provider/catalog` → `ProviderCatalogDto`

Provider catalog entries expose the same product facts for catalog/model picker
surfaces:

```json
{
  "schema": "provider_catalog.v1",
  "providers": [
    {
      "provider_id": "string",
      "label": "string",
      "enabled": "bool",
      "source": "runtime | env | config | builtin",
      "base_url_host": "string",
      "default_model": "string",
      "available_model_ids": ["string"],
      "context_limit": "u64 | null",
      "output_limit": "u64 | null",
      "auto_compact_threshold": "u64 | null",
      "token_counter": "string",
      "cache_accounting": "string",
      "protocol_family": "string",
      "supports_streaming": "bool",
      "requires_nonstreaming": "bool",
      "tool_schema_transform": "string",
      "prompt_delta": "string",
      "request_timeout_secs": "u64",
      "stream_idle_timeout_secs": "u64",
      "cost_cache_write_per_1m": "f64 | null"
    }
  ]
}
```

---

## Session Revert Item

`GET /api/sessions/:id/reverts` returns a page of `SessionRevertItem` payloads
read from the durable `session_reverts` projection (not only from raw events).

```json
{
  "session_id": "string",
  "reverts": [
    {
      "status": "completed | partial | failed",
      "message_id": "string | null",
      "target_part_id": "string | null",
      "part_ids": ["string"],
      "paths": ["string"],
      "restored_files": ["string"],
      "removed_files": ["string"],
      "errors": ["string"],
      "snapshot_checkpoint_id": "string | null",
      "timestamp": "datetime | null",
      "unrevert_possible": "bool"
    }
  ],
  "total": "usize"
}
```

---

## API Chat vs Full-Agent

- `POST /api/provider-chat` is the explicit **provider chat** route. It sends a
  message to the configured LLM provider directly. It does **not** enter the
  full programming-agent runtime (no tool loop, no checkpoint, no closeout
  proof, no session event stream). The response includes
  `execution_kind: "provider_chat"` and `full_agent: false`.
- `POST /api/chat` is a legacy alias for provider chat. It also returns
  `execution_kind: "provider_chat"` and `full_agent: false`, plus
  `deprecated_route: "/api/chat"` and `replacement_route: "/api/provider-chat"`.
- The full-agent prompt path for HTTP is `POST /api/sessions/:id/prompt`
  with `execution_kind: "full_agent_turn"` and
  `agent_runtime_entrypoint: "RuntimeController"`. Production API startup
  injects a `RuntimeController`-backed `ApiAgentRuntime`; if a custom/test
  `ApiState` is constructed without one, the route returns an honest typed
  `501`. Tests can inject a fake runtime and receive `accepted: true`.
- Full-agent prompt delivery:
  - `delivery: "run"` admits the prompt and executes it through
    `RuntimeController`.
  - `delivery: "queue"` durably admits the prompt into `session_inputs` and
    returns `202 Accepted`; the API runtime schedules a background drain task.
  - `delivery: "admit_only"` durably admits the prompt without starting
    execution.
  - `idempotency_key` is stored as `session_inputs.prompt_id`; same key/content
    is an idempotent replay, while same key/different content returns
    `409 Conflict`.
  - Current API drain is globally serialized because the Rust runtime still uses
    one shared `RuntimeController`. True per-session runner handles are the next
    hardening layer.

---

## Runtime Config

`GET /api/config` returns user-facing config plus runtime discoverability fields:

```json
{
  "api": {
    "model": "string",
    "base_url": "string",
    "temperature": "f32",
    "max_tokens": "u32 | null"
  },
  "ui": {
    "theme": "string",
    "show_token_usage": "bool"
  },
  "features": {
    "mcp_enabled": "bool",
    "skills_enabled": "bool",
    "web_search": "bool"
  },
  "runtime": {
    "full_agent_prompt_available": "bool",
    "agent_runtime_entrypoint": "RuntimeController | null",
    "session_prompt_endpoint": "/api/sessions/{id}/prompt"
  },
  "context": {
    "provider_family": "string",
    "model_pattern": "string",
    "context_window_tokens": "u64",
    "reserved_output_tokens": "u64",
    "auto_compact_threshold_tokens": "u64",
    "token_counter": "tiktoken:o200k_base | tiktoken:cl100k_base | heuristic:*",
    "cache_accounting": "string",
    "background_prune_enabled": "bool",
    "time_based_compression_enabled": "bool",
    "context_collapse_enabled": "bool",
    "llm_compaction_enabled": "bool"
  }
}
```

`full_agent_prompt_available` is the API product-readiness check for
`POST /api/sessions/:id/prompt`: production API startup injects
`RuntimeController`; tests or custom states may intentionally omit it and return
typed `501`.

---

## Cursor Semantics

All cursor-based routes include `cursor.limit` (the requested page size),
`cursor.has_more` (whether additional pages exist beyond the returned items),
and a next-cursor field (`after_part_index` or `after_seq`) to use on the next
request.  Clients should drive paging with `after` + `limit`, not by guessing
indices.

- `session_parts` is the **preferred UI projection** (typed, stable part_ids).
- `session_events` is the **audit/debug stream** (raw event entries, append-only,
  use only when you need replay fidelity).

---

## Original Schema (pre-v2)

The sections below describe the earlier schema versions and remain valid for
the baseline `run_report.v1` diagnostic export format.

Previous Provider Runtime Profile (single profile, deprecated in favor of
`ProviderStatusPage`):
  "last_timeout_category": "string | null",
  "capability_summary": "string"
}
```

API:
- `ProviderRuntimeProfile::snapshot(capabilities, model, provider_id)` → snapshot
- `diagnostics::provider_health::provider_health_ledger_path()` →
  `provider-health.jsonl`
- `/provider status --json` → current provider/model/runtime facts plus latest
  provider health ledger entry

Desktop provider status DTO:

```json
{
  "active_provider": "string | null",
  "active_provider_label": "string | null",
  "active_model": "string",
  "active_base_url": "string",
  "runtime_model": "string | null",
  "runtime_provider_ready": "bool",
  "selection_source": "runtime | desktop_settings | environment | preview",
  "configured_count": "usize",
  "providers": ["DesktopProviderOption"],
  "models": ["DesktopModelOption"]
}
```

---

## Diagnostic Export

```json
{
  "version": "run_report.v1",
  "session_id": "string",
  "exported_at": "datetime",
  "session_parts_count": "usize",
  "session_events_count": "usize",
  "revert_events": "usize",
  "usage_totals": {
    "prompt_tokens": "u64",
    "completion_tokens": "u64",
    "cached_tokens": "u64",
    "cache_write_tokens": "u64"
  },
  "provider_profile": "<ProviderRuntimeProfile>",
  "tool_output_policy": {
    "max_bytes": "usize",
    "max_lines": "usize",
    "preview_direction": "head | tail | head_tail",
    "retention_days": "u32"
  },
  "changed_files": "usize",
  "tool_rounds": "usize",
  "failed_tools": "usize"
}
```

---

## Feature Gate

All API DTOs are available under the `experimental-api-server` feature flag.
Without this flag, only the internal session-store APIs (used by TUI/desktop)
are available.

```bash
cargo check --features experimental-api-server -q
```

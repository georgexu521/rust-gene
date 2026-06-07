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
      "configured_max_output": "u64 | null",
      "cost_input_per_1m": "f64 | null",
      "cost_output_per_1m": "f64 | null",
      "cost_cache_read_per_1m": "f64 | null",
      "latest_health_status": "ok | failed | null",
      "latest_timeout_category": "string | null",
      "last_request_latency_ms": "u64 | null",
      "request_timeout_secs": "u64",
      "stream_idle_timeout_secs": "u64",
      "capability_summary": "string"
    }
  ],
  "record_count": "usize"
}
```

API:
- `GET /api/provider/status` → `ProviderStatusPage`

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

- `POST /api/chat` is **provider chat only** — it sends a message to the
  configured LLM provider directly.  It does **not** enter the full
  programming-agent runtime (no tool loop, no checkpoint, no closeout proof,
  no session event stream).  The response includes `execution_kind: "provider_chat"`
  and `full_agent: false` to make this unambiguous.
- The full-agent prompt path for HTTP is `POST /api/sessions/:id/prompt`
  (feature-gated, returns typed `501` until wired to `RuntimeController`).

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
    "cached_tokens": "u64"
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

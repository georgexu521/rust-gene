# Session API DTO Schema

Stable data-transfer objects shared by TUI, desktop, and the experimental API
server.  These DTOs are the canonical vocabulary for session state — frontends
consume them directly rather than re-interpreting raw trace internals.

Status: P2 — DTOs defined, experimental API feature-gated, desktop integration
in progress.

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

## Provider Runtime Profile

```json
{
  "provider_id": "string",
  "model_id": "string",
  "protocol_family": "openai_compatible | minimax | kimi | anthropic_like | reasoning_capable",
  "supports_streaming_tool_calls": "bool",
  "requires_nonstreaming_tool_calls": "bool",
  "request_timeout_secs": "u64",
  "stream_idle_timeout_secs": "u64",
  "last_health_status": "string | null",
  "last_timeout_category": "string | null",
  "capability_summary": "string"
}
```

API:
- `ProviderRuntimeProfile::snapshot(capabilities, model, provider_id)` → snapshot

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

#!/usr/bin/env bash
set -euo pipefail

# Reads hook payload JSON from stdin and denies tool calls not in the allowlist.
# Allowlist can be overridden by PRIORITY_AGENT_ALLOWED_TOOLS (comma-separated).

payload="$(cat)"
if [[ -z "${payload}" ]]; then
  exit 0
fi

tool_name="$(python3 -c 'import json,sys; data=json.load(sys.stdin); print(data.get("tool_name",""))' <<<"${payload}" 2>/dev/null || true)"
if [[ -z "${tool_name}" ]]; then
  exit 0
fi

allowlist_csv="${PRIORITY_AGENT_ALLOWED_TOOLS:-file_read,glob,grep,project_list,web_search,web_fetch,memory_load,skills_list,skill_view,todo_write}"
allowlist_csv="${allowlist_csv// /}"

IFS=',' read -r -a allowlist <<<"${allowlist_csv}"
for allowed in "${allowlist[@]}"; do
  if [[ "${tool_name}" == "${allowed}" ]]; then
    exit 0
  fi
done

printf '{"allow":false,"reason":"Tool '\''%s'\'' is blocked by CI whitelist"}\n' "${tool_name}"


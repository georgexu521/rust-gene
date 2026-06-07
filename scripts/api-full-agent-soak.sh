#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PORT="${PRIORITY_AGENT_API_SOAK_PORT:-18787}"
TOKEN="${PRIORITY_AGENT_BRIDGE_TOKEN:-api-soak-token}"
SESSION_ID="${PRIORITY_AGENT_API_SOAK_SESSION_ID:-api-soak-$(date +%s)}"
IDEMPOTENCY_KEY="${PRIORITY_AGENT_API_SOAK_IDEMPOTENCY_KEY:-api-soak-${SESSION_ID}}"
PROMPT="${PRIORITY_AGENT_API_SOAK_PROMPT:-Inspect the project briefly and reply with one concise sentence. Do not modify files.}"
QUEUE_PROMPT="${PRIORITY_AGENT_API_SOAK_QUEUE_PROMPT:-Reply with one concise sentence confirming queued API execution. Do not modify files.}"
RUN_QUEUE_CHECK="${PRIORITY_AGENT_API_SOAK_QUEUE:-0}"
LOG_FILE="${PRIORITY_AGENT_API_SOAK_LOG:-target/api-full-agent-soak.log}"
BASE_URL="http://127.0.0.1:${PORT}"

mkdir -p "$(dirname "$LOG_FILE")"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 2
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 2
fi

export PRIORITY_AGENT_BRIDGE_TOKEN="$TOKEN"
export PROMPT
export IDEMPOTENCY_KEY
export QUEUE_PROMPT

cargo run --features experimental-api-server -- --api --port "$PORT" >"$LOG_FILE" 2>&1 &
SERVER_PID="$!"

cleanup() {
  if kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

for _ in $(seq 1 60); do
  if curl -fsS -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

curl -fsS -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/health" >/dev/null

REQUEST_JSON="$(python3 - <<'PY'
import json
import os

print(json.dumps({
    "message": os.environ["PROMPT"],
    "agent_mode": "explore",
    "stream": False,
    "delivery": "run",
    "idempotency_key": os.environ["IDEMPOTENCY_KEY"],
}))
PY
)"

RESPONSE_FILE="$(mktemp)"
HTTP_STATUS="$(
  curl -sS -o "$RESPONSE_FILE" -w "%{http_code}" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -X POST \
    --data "$REQUEST_JSON" \
    "${BASE_URL}/api/sessions/${SESSION_ID}/prompt"
)"

python3 - "$HTTP_STATUS" "$RESPONSE_FILE" <<'PY'
import json
import sys

status = int(sys.argv[1])
path = sys.argv[2]
with open(path, "r", encoding="utf-8") as f:
    payload = json.load(f)

if status != 200:
    raise SystemExit(f"prompt returned HTTP {status}: {payload}")
if payload.get("execution_kind") != "full_agent_turn":
    raise SystemExit(f"wrong execution_kind: {payload}")
if payload.get("agent_runtime_entrypoint") != "RuntimeController":
    raise SystemExit(f"wrong runtime entrypoint: {payload}")
if not payload.get("accepted"):
    raise SystemExit(f"prompt was not accepted: {payload}")
if int(payload.get("events_written") or 0) <= 0:
    raise SystemExit(f"no stream events were written: {payload}")
print(json.dumps(payload, ensure_ascii=False, indent=2))
PY

PARTS_FILE="$(mktemp)"
curl -fsS \
  -H "Authorization: Bearer ${TOKEN}" \
  "${BASE_URL}/api/sessions/${SESSION_ID}/parts?limit=20" >"$PARTS_FILE"

python3 - "$PARTS_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    payload = json.load(f)

parts = payload.get("parts") or []
if not parts:
    raise SystemExit(f"session parts were empty: {payload}")
print(f"session_parts={len(parts)}")
PY

if [[ "$RUN_QUEUE_CHECK" == "1" || "$RUN_QUEUE_CHECK" == "true" ]]; then
  EVENTS_FILE="$(mktemp)"
  curl -fsS \
    -H "Authorization: Bearer ${TOKEN}" \
    "${BASE_URL}/api/sessions/${SESSION_ID}/events?limit=200" >"$EVENTS_FILE"
  INITIAL_EVENTS="$(
    python3 - "$EVENTS_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    payload = json.load(f)
events = payload.get("events") or []
print(events[-1]["seq"] if events else 0)
PY
  )"
  QUEUE_KEY="${IDEMPOTENCY_KEY}-queue"
  export QUEUE_KEY
  QUEUE_JSON="$(python3 - <<'PY'
import json
import os

print(json.dumps({
    "message": os.environ["QUEUE_PROMPT"],
    "agent_mode": "explore",
    "stream": False,
    "delivery": "queue",
    "idempotency_key": os.environ["QUEUE_KEY"],
}))
PY
)"
  QUEUE_RESPONSE_FILE="$(mktemp)"
  QUEUE_STATUS="$(
    curl -sS -o "$QUEUE_RESPONSE_FILE" -w "%{http_code}" \
      -H "Authorization: Bearer ${TOKEN}" \
      -H "Content-Type: application/json" \
      -X POST \
      --data "$QUEUE_JSON" \
      "${BASE_URL}/api/sessions/${SESSION_ID}/prompt"
  )"
  python3 - "$QUEUE_STATUS" "$QUEUE_RESPONSE_FILE" <<'PY'
import json
import sys

status = int(sys.argv[1])
path = sys.argv[2]
with open(path, "r", encoding="utf-8") as f:
    payload = json.load(f)
if status != 202:
    raise SystemExit(f"queue returned HTTP {status}: {payload}")
if payload.get("status") != "queued":
    raise SystemExit(f"queue response was not queued: {payload}")
print(json.dumps(payload, ensure_ascii=False, indent=2))
PY

  QUEUE_EVENTS_FILE="$(mktemp)"
  for _ in $(seq 1 90); do
    curl -fsS \
      -H "Authorization: Bearer ${TOKEN}" \
      "${BASE_URL}/api/sessions/${SESSION_ID}/events?limit=200" >"$QUEUE_EVENTS_FILE"
    CURRENT_EVENTS="$(python3 - "$QUEUE_EVENTS_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    payload = json.load(f)
events = payload.get("events") or []
print(events[-1]["seq"] if events else 0)
PY
    )"
    if [[ "$CURRENT_EVENTS" -gt "$INITIAL_EVENTS" ]]; then
      echo "queue_session_events=${CURRENT_EVENTS}"
      break
    fi
    sleep 1
  done
  if [[ "${CURRENT_EVENTS:-0}" -le "$INITIAL_EVENTS" ]]; then
    echo "queued prompt did not increase session events within timeout" >&2
    exit 1
  fi
fi

echo "API full-agent soak passed for session ${SESSION_ID}"

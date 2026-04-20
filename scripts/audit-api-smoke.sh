#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
EXPORT_PATH="${EXPORT_PATH:-/tmp/priority-agent-audit-smoke.json}"

echo "[1/6] Create session..."
SESSION_JSON="$(curl -sS -X POST "$BASE_URL/api/sessions" \
  -H "Content-Type: application/json" \
  -d '{"title":"audit-smoke"}')"
SESSION_ID="$(printf "%s" "$SESSION_JSON" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"
if [[ -z "$SESSION_ID" ]]; then
  echo "Failed to parse session id from response:"
  echo "$SESSION_JSON"
  exit 1
fi
echo "session_id=$SESSION_ID"

echo "[2/6] Call tool success (bash echo)..."
curl -sS -X POST "$BASE_URL/api/tools/call" \
  -H "Content-Type: application/json" \
  -d "{\"tool\":\"bash\",\"session_id\":\"$SESSION_ID\",\"params\":{\"command\":\"echo audit-smoke\",\"backend\":\"local\"}}" >/dev/null

echo "[3/6] Call tool failure (bash exit 1)..."
curl -sS -X POST "$BASE_URL/api/tools/call" \
  -H "Content-Type: application/json" \
  -d "{\"tool\":\"bash\",\"session_id\":\"$SESSION_ID\",\"params\":{\"command\":\"exit 1\",\"backend\":\"local\"}}" >/dev/null

echo "[4/6] Get /api/audit/summary..."
SUMMARY="$(curl -sS "$BASE_URL/api/audit/summary")"
echo "$SUMMARY"

echo "[5/6] Get /api/audit/recent?limit=10..."
RECENT="$(curl -sS "$BASE_URL/api/audit/recent?limit=10")"
echo "$RECENT"

echo "[6/6] Export /api/audit/export..."
EXPORT="$(curl -sS -X POST "$BASE_URL/api/audit/export" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION_ID\",\"recent_limit\":100,\"path\":\"$EXPORT_PATH\"}")"
echo "$EXPORT"

if [[ ! -f "$EXPORT_PATH" ]]; then
  echo "Expected export file not found: $EXPORT_PATH"
  exit 1
fi

echo "Smoke test passed. Export file: $EXPORT_PATH"

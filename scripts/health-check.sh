#!/usr/bin/env bash
# Priority Agent API Server 健康检查脚本
# Usage: ./scripts/health-check.sh [--port PORT] [--wait]

set -euo pipefail

PORT="${PRIORITY_AGENT_PORT:-8787}"
HOST="${PRIORITY_AGENT_HOST:-127.0.0.1}"
WAIT_MODE=0
TIMEOUT_SECS=30

usage() {
  cat <<'EOF'
Usage: scripts/health-check.sh [options]

Options:
  --port PORT     API server port (default: 8787, or $PRIORITY_AGENT_PORT)
  --host HOST     API server host (default: 127.0.0.1)
  --wait          Wait up to 30s for server to become healthy
  --timeout N     Wait timeout in seconds (default: 30)
  -h, --help      Show this help

Exit codes:
  0  Healthy
  1  Unhealthy or unreachable
  2  Usage error
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="${2:-}"; shift 2 ;;
    --host) HOST="${2:-}"; shift 2 ;;
    --wait) WAIT_MODE=1; shift ;;
    --timeout) TIMEOUT_SECS="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

HEALTH_URL="http://${HOST}:${PORT}/api/health"

check_once() {
  local url="$1"
  local response
  local http_code
  response=$(curl -fsS -o /dev/null -w "%{http_code}" "$url" 2>/dev/null) || true
  if [[ "$response" == "200" ]]; then
    return 0
  fi
  return 1
}

if [[ "$WAIT_MODE" -eq 1 ]]; then
  echo "Waiting for Priority Agent API at $HEALTH_URL (timeout: ${TIMEOUT_SECS}s)..."
  for ((i=1; i<=TIMEOUT_SECS; i++)); do
    if check_once "$HEALTH_URL"; then
      echo "OK: API server is healthy (${i}s)"
      exit 0
    fi
    if [[ "$i" -lt "$TIMEOUT_SECS" ]]; then
      sleep 1
    fi
  done
  echo "FAIL: API server did not become healthy within ${TIMEOUT_SECS}s"
  exit 1
else
  if check_once "$HEALTH_URL"; then
    echo "OK: API server at $HEALTH_URL is healthy"
    exit 0
  else
    echo "FAIL: API server at $HEALTH_URL is unreachable or unhealthy"
    exit 1
  fi
fi

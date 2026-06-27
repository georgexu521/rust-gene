#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT_DIR/apps/desktop/src-tauri/tauri.conf.json"

if grep -q "http://127\\.0\\.0\\.1:\\*" "$TAURI_CONF"; then
  echo "desktop release security check failed: production CSP contains http://127.0.0.1:*" >&2
  exit 1
fi

if ! grep -q "connect-src ipc:" "$TAURI_CONF"; then
  echo "desktop release security check failed: production CSP must allow ipc:" >&2
  exit 1
fi

echo "desktop release security check passed"

#!/usr/bin/env bash
set -euo pipefail

STRICT=false

usage() {
  cat <<'USAGE'
Usage: scripts/macos-release-preflight.sh [--strict]

Check whether this Mac has the tools and credentials needed for a future
Developer ID distribution of the Priority Agent desktop app.

Environment:
  PRIORITY_AGENT_DEVELOPER_ID       Optional expected Developer ID Application identity.
  PRIORITY_AGENT_NOTARY_PROFILE     Optional notarytool keychain profile name.

Options:
  --strict    Exit non-zero when Developer ID or notary profile checks are missing.
  -h, --help  Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --strict)
      STRICT=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

WARNINGS=0

ok() {
  printf 'ok      %s\n' "$1"
}

warn() {
  WARNINGS=$((WARNINGS + 1))
  printf 'warning %s\n' "$1"
}

fail() {
  printf 'error   %s\n' "$1" >&2
  exit 1
}

require_command() {
  local command_name="$1"
  local label="$2"
  if command -v "$command_name" >/dev/null 2>&1; then
    ok "$label: $(command -v "$command_name")"
  else
    fail "$label is missing: $command_name"
  fi
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  fail "macOS release preflight must run on macOS"
fi

require_command corepack "Corepack"
require_command cargo "Cargo"
require_command codesign "codesign"
require_command xcrun "xcrun"
require_command hdiutil "hdiutil"

if xcrun -f notarytool >/dev/null 2>&1; then
  ok "notarytool is available: $(xcrun -f notarytool)"
else
  fail "xcrun notarytool is unavailable"
fi

if xcrun -f stapler >/dev/null 2>&1; then
  ok "stapler is available: $(xcrun -f stapler)"
else
  fail "xcrun stapler is unavailable"
fi

IDENTITIES="$(security find-identity -v -p codesigning 2>/dev/null || true)"
EXPECTED_IDENTITY="${PRIORITY_AGENT_DEVELOPER_ID:-}"
if [[ -n "$EXPECTED_IDENTITY" ]]; then
  if grep -Fq "$EXPECTED_IDENTITY" <<<"$IDENTITIES"; then
    ok "Developer ID identity found: $EXPECTED_IDENTITY"
  else
    warn "Developer ID identity not found: $EXPECTED_IDENTITY"
  fi
elif grep -Fq "Developer ID Application" <<<"$IDENTITIES"; then
  ok "At least one Developer ID Application identity is installed"
else
  warn "No Developer ID Application identity found in the login keychain"
fi

if [[ -n "${PRIORITY_AGENT_NOTARY_PROFILE:-}" ]]; then
  ok "notarytool keychain profile configured: $PRIORITY_AGENT_NOTARY_PROFILE"
else
  warn "PRIORITY_AGENT_NOTARY_PROFILE is not set"
fi

if [[ "$STRICT" == true && "$WARNINGS" -gt 0 ]]; then
  fail "release preflight has $WARNINGS warning(s)"
fi

echo "macOS release preflight completed with $WARNINGS warning(s)"

#!/usr/bin/env bash
# Run non-noisy dependency and license checks for release candidates.
# This script never opens dependency PRs or creates GitHub branches.

set -euo pipefail

missing=()
if ! command -v cargo-audit >/dev/null 2>&1; then
  missing+=("cargo-audit")
fi
if ! command -v cargo-deny >/dev/null 2>&1; then
  missing+=("cargo-deny")
fi

if (( ${#missing[@]} > 0 )); then
  echo "Missing security audit tools: ${missing[*]}" >&2
  echo "" >&2
  echo "Install them with:" >&2
  echo "  cargo install cargo-audit cargo-deny" >&2
  exit 2
fi

echo "== cargo audit =="
cargo audit

echo ""
echo "== cargo deny check =="
cargo deny check

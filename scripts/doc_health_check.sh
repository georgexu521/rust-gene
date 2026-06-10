#!/usr/bin/env bash
# Doc health check: ensure documentation stays manageable.
#
# Run: bash scripts/doc_health_check.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DOCS_DIR="$PROJECT_ROOT/docs"

failures=0

# 1. Count of docs/*.md must stay under 50
active_count=$(ls "$DOCS_DIR"/*.md 2>/dev/null | wc -l | tr -d ' ')
if [ "$active_count" -ge 50 ]; then
    echo "FAIL: docs/*.md has $active_count files (max 49)"
    failures=$((failures + 1))
else
    echo "PASS: docs/*.md has $active_count files (under 50)"
fi

# 2. No plan doc older than 90 days without a Status header
cutoff_ts=$(date -j -v-90d +%s 2>/dev/null || date -d '90 days ago' +%s)
for f in "$DOCS_DIR"/*.md; do
    basename="$(basename "$f")"
    # Skip README, index, and non-plan docs
    case "$basename" in
        README.md|PROJECT_MAP.md|CAPABILITY_LADDER.md|CONTROLLER_INDEX.md|SKILL_ROOTS_AND_TRUST.md|SOUL_USER_TOOLS_CONTEXT.md|PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026*.md)
            continue ;;
    esac
    # Check for Date header
    if ! grep -qE '^Date: ' "$f" 2>/dev/null; then
        mtime_ts=$(stat -f %m "$f" 2>/dev/null || stat -c %Y "$f")
        if [ "$mtime_ts" -lt "$cutoff_ts" ]; then
            echo "WARN: $basename is older than 90 days and has no Date header"
        fi
    fi
    if ! grep -qE '^Status: ' "$f" 2>/dev/null; then
        echo "WARN: $basename has no Status header"
    fi
done

# 3. No archived/ merged/ dirs under docs/ itself
for d in "$DOCS_DIR"/archive "$DOCS_DIR"/merged; do
    if [ -d "$d" ]; then
        echo "INFO: $d exists ($(ls "$d"/*.md 2>/dev/null | wc -l | tr -d ' ') archived files)"
    fi
done

if [ "$failures" -gt 0 ]; then
    echo "FAIL: $failures check(s) failed"
    exit 1
else
    echo "PASS: doc health OK"
fi

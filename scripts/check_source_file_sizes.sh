#!/usr/bin/env bash
# Report non-test Rust production files that exceed the project line ceiling.

set -euo pipefail

LIMIT="${SOURCE_FILE_LINE_LIMIT:-1500}"

echo "Checking Rust production file line ceiling (limit: ${LIMIT})..."

oversized=$(
    find src \
        -path '*/tests.rs' -prune -o \
        -path '*/tests/*' -prune -o \
        -name '*.rs' -type f -print \
        | xargs wc -l \
        | awk -v limit="$LIMIT" '$2 != "total" && $1 > limit { print }'
)

if [[ -n "$oversized" ]]; then
    echo "  [FAIL] Production Rust files exceed ${LIMIT} lines:"
    echo "$oversized" | sed 's/^/    /'
    exit 1
fi

echo "  [OK] No production Rust file exceeds ${LIMIT} lines"

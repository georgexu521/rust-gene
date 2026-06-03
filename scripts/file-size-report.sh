#!/usr/bin/env bash
# Report large source files while excluding generated/build/dependency output.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

THRESHOLD=1000
TOP=0
FORMAT="text"
FAIL_OVER=0

usage() {
  cat <<'EOF'
Usage: scripts/file-size-report.sh [options]

Options:
  --threshold N   Only show files above N lines (default: 1000)
  --top N         Show the top N largest files after filtering
  --fail-over N   Exit 1 when any included file is above N lines
  --json          Emit JSON instead of text
  -h, --help      Show this help

Generated, dependency, build, docs, evalset, and benchmark artifact directories
are excluded by default.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --threshold) THRESHOLD="${2:-1000}"; shift 2 ;;
    --top) TOP="${2:-0}"; shift 2 ;;
    --fail-over) FAIL_OVER="${2:-0}"; shift 2 ;;
    --json) FORMAT="json"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

python3 - "$THRESHOLD" "$TOP" "$FORMAT" "$FAIL_OVER" <<'PY'
import json
import pathlib
import sys
from collections import Counter

threshold = int(sys.argv[1])
top = int(sys.argv[2])
output_format = sys.argv[3]
fail_over = int(sys.argv[4])

skip_parts = {
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".next",
    "coverage",
    "docs",
    "evalsets",
    "__pycache__",
}
suffixes = {
    ".rs",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".py",
    ".sh",
    ".toml",
    ".yaml",
    ".yml",
}
areas = (
    "src/",
    "apps/desktop/src/",
    "apps/desktop/src-tauri/src/",
    "scripts/",
    "tests/",
)


def area_for(path: str) -> str:
    for area in areas:
        if path.startswith(area):
            return area.rstrip("/")
    return "other"


def category_for(path: str) -> str:
    if path.endswith("/tests.rs") or "/tests/" in path or path.startswith("tests/"):
        return "rust_test"
    if path.startswith("scripts/"):
        return "script"
    if path.startswith("apps/desktop/src/") and path.endswith((".ts", ".tsx", ".js", ".jsx")):
        return "desktop_frontend"
    if path.startswith("apps/desktop/src-tauri/src/"):
        return "desktop_tauri"
    if path.startswith("src/tools/"):
        return "tool_runtime"
    if path.startswith("src/tui/"):
        return "tui_runtime"
    if path.startswith("src/memory/"):
        return "memory_runtime"
    if path.startswith("src/engine/"):
        return "engine_runtime"
    if path.startswith("src/"):
        return "runtime"
    return "other"


def action_for(line_count: int, category: str) -> str:
    if line_count > 3000:
        return "priority_split"
    if category == "rust_test":
        return "test_exception" if line_count > 1500 else "test_watch"
    if line_count > 2000:
        return "split_plan"
    if line_count > 1500:
        return "split_candidate"
    return "watch"


rows = []
for path in pathlib.Path(".").rglob("*"):
    if not path.is_file():
        continue
    if any(part in skip_parts for part in path.parts):
        continue
    if path.suffix not in suffixes:
        continue
    try:
        line_count = sum(1 for _ in path.open("r", encoding="utf-8", errors="ignore"))
    except OSError:
        continue
    if line_count >= threshold:
        normalized = path.as_posix()
        category = category_for(normalized)
        rows.append(
            {
                "lines": line_count,
                "path": normalized,
                "area": area_for(normalized),
                "category": category,
                "action": action_for(line_count, category),
            }
        )

rows.sort(key=lambda item: (-item["lines"], item["path"]))
all_rows = rows
visible_rows = rows[:top] if top > 0 else rows
category_counts = Counter(item["category"] for item in all_rows)
action_counts = Counter(item["action"] for item in all_rows)
failures = [item for item in all_rows if fail_over > 0 and item["lines"] > fail_over]

if output_format == "json":
    print(
        json.dumps(
            {
                "threshold": threshold,
                "count": len(all_rows),
                "shown": len(visible_rows),
                "fail_over": fail_over,
                "failure_count": len(failures),
                "categories": dict(sorted(category_counts.items())),
                "actions": dict(sorted(action_counts.items())),
                "files": visible_rows,
            },
            indent=2,
        )
    )
else:
    print(f"threshold: {threshold}")
    print(f"files: {len(all_rows)}")
    if top > 0:
        print(f"shown: {len(visible_rows)}")
    if fail_over > 0:
        print(f"fail_over: {fail_over}")
        print(f"failure_count: {len(failures)}")
    print()
    print("categories:")
    for category, count in sorted(category_counts.items()):
        print(f"  {category}: {count}")
    print()
    print(f"{'lines':>6}  {'action':<16}  {'category':<17}  path")
    print(f"{'-' * 6}  {'-' * 16}  {'-' * 17}  {'-' * 40}")
    for row in visible_rows:
        print(f"{row['lines']:>6}  {row['action']:<16}  {row['category']:<17}  {row['path']}")

if failures:
    print(
        f"file-size-report: {len(failures)} file(s) exceed --fail-over {fail_over}",
        file=sys.stderr,
    )
    sys.exit(1)
PY

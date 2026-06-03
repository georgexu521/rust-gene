#!/usr/bin/env bash
# Report large source files while excluding generated/build/dependency output.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

THRESHOLD=1000
TOP=0
FORMAT="text"

usage() {
  cat <<'EOF'
Usage: scripts/file-size-report.sh [options]

Options:
  --threshold N   Only show files above N lines (default: 1000)
  --top N         Show the top N largest files after filtering
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
    --json) FORMAT="json"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

python3 - "$THRESHOLD" "$TOP" "$FORMAT" <<'PY'
import json
import pathlib
import sys

threshold = int(sys.argv[1])
top = int(sys.argv[2])
output_format = sys.argv[3]

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
        rows.append(
            {
                "lines": line_count,
                "path": normalized,
                "area": area_for(normalized),
                "action": (
                    "priority_split"
                    if line_count > 3000
                    else "split_plan"
                    if line_count > 1500
                    else "watch"
                ),
            }
        )

rows.sort(key=lambda item: (-item["lines"], item["path"]))
if top > 0:
    rows = rows[:top]

if output_format == "json":
    print(json.dumps({"threshold": threshold, "count": len(rows), "files": rows}, indent=2))
else:
    print(f"threshold: {threshold}")
    print(f"files: {len(rows)}")
    print()
    print(f"{'lines':>6}  {'action':<14}  path")
    print(f"{'-' * 6}  {'-' * 14}  {'-' * 40}")
    for row in rows:
        print(f"{row['lines']:>6}  {row['action']:<14}  {row['path']}")
PY

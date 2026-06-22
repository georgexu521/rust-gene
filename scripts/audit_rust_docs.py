#!/usr/bin/env python3
"""Advisory rustdoc audit for release-facing code documentation.

The audit intentionally stays lightweight. It reports likely missing module
docs and likely missing docs for `pub` / `pub(crate)` items, excluding tests and
generated-like fixtures. It is not a Rust parser and should be treated as a
maintainer signal, not as semantic proof.
"""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import sys
from collections import Counter


ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

SKIP_PARTS = {
    "tests",
    "test_utils",
}

ITEM_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)"
    r"(?:(?:async|const|unsafe|extern)\s+)*"
    r"(struct|enum|trait|type|const|static|fn|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
)


@dataclasses.dataclass(frozen=True)
class Finding:
    kind: str
    path: pathlib.Path
    line: int
    name: str

    def format(self) -> str:
        rel = self.path.relative_to(ROOT)
        return f"{self.kind}: {rel}:{self.line}: {self.name}"


def is_skipped(path: pathlib.Path) -> bool:
    parts = set(path.relative_to(ROOT).parts)
    if parts & SKIP_PARTS:
        return True
    name = path.name
    return name == "tests.rs" or name.endswith("_tests.rs")


def has_module_doc(lines: list[str]) -> bool:
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("#!") or stripped.startswith("//!") or stripped.startswith("//"):
            if stripped.startswith("//!"):
                return True
            continue
        return False
    return False


def has_item_doc(lines: list[str], idx: int) -> bool:
    j = idx - 1
    while j >= 0:
        stripped = lines[j].strip()
        if not stripped:
            j -= 1
            continue
        if stripped.startswith("#["):
            j -= 1
            continue
        return stripped.startswith("///")
    return False


def scan_file(path: pathlib.Path) -> list[Finding]:
    lines = path.read_text(encoding="utf-8").splitlines()
    findings: list[Finding] = []
    if not has_module_doc(lines):
        findings.append(Finding("missing-module-doc", path, 1, path.stem))

    for idx, line in enumerate(lines):
        match = ITEM_RE.match(line)
        if not match:
            continue
        item_kind, name = match.groups()
        if item_kind == "mod" and line.strip().endswith(";"):
            continue
        if not has_item_doc(lines, idx):
            findings.append(
                Finding("missing-item-doc", path, idx + 1, f"{item_kind} {name}")
            )
    return findings


def rust_files() -> list[pathlib.Path]:
    return sorted(
        path for path in SRC.rglob("*.rs") if path.is_file() and not is_skipped(path)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fail", action="store_true", help="exit non-zero when findings exist")
    parser.add_argument("--limit", type=int, default=80, help="maximum findings to print")
    args = parser.parse_args()

    findings: list[Finding] = []
    for path in rust_files():
        findings.extend(scan_file(path))

    counts = Counter(f.kind for f in findings)
    by_file = Counter(f.path for f in findings)

    print("Rust documentation audit")
    print(f"  files scanned: {len(rust_files())}")
    print(f"  findings: {len(findings)}")
    for kind in sorted(counts):
        print(f"  {kind}: {counts[kind]}")

    if findings:
        print("\nTop files:")
        for path, count in by_file.most_common(20):
            print(f"  {path.relative_to(ROOT)}: {count}")
        print(f"\nFirst {min(args.limit, len(findings))} findings:")
        for finding in findings[: args.limit]:
            print(f"  {finding.format()}")

    return 1 if args.fail and findings else 0


if __name__ == "__main__":
    sys.exit(main())

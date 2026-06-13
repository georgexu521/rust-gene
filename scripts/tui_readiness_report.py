#!/usr/bin/env python3
"""Summarize TUI tool-turn PTY matrix results into a readiness report."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


CONTRACT_KEYS = (
    "session_event_contract",
    "terminal_contract",
    "persistence_contract",
    "projection_contract",
    "provider_repair_diagnostic_contract",
)


@dataclass(frozen=True)
class MatrixSpec:
    name: str
    path: Path


def parse_matrix(value: str) -> MatrixSpec:
    if "=" not in value:
        raise argparse.ArgumentTypeError("matrix must use NAME=PATH")
    name, path = value.split("=", 1)
    name = name.strip()
    if not name:
        raise argparse.ArgumentTypeError("matrix name cannot be empty")
    return MatrixSpec(name=name, path=Path(path).expanduser())


def load_json(path: Path) -> list[dict[str, Any]]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"{path}: invalid JSON: {exc}") from exc
    if not isinstance(payload, list):
        raise ValueError(f"{path}: expected top-level JSON array")
    rows: list[dict[str, Any]] = []
    for index, item in enumerate(payload):
        if not isinstance(item, dict):
            raise ValueError(f"{path}: entry {index} is not an object")
        rows.append(item)
    return rows


def contract_failures(row: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    for key in CONTRACT_KEYS:
        value = row.get(key)
        if value is not None and value != "passed":
            error_key = key.replace("_contract", "_error")
            detail = row.get(error_key)
            failures.append(f"{key}={value}" + (f" ({detail})" if detail else ""))
    if row.get("sent_prompt") is False:
        failures.append("prompt was not sent")
    if row.get("saw_raw_async_openai"):
        failures.append("raw async_openai error leaked to terminal")
    if row.get("saw_deser"):
        failures.append("provider deserialization error leaked to terminal")
    if row.get("saw_display_provider") is False:
        failures.append("expected provider label was not visible")
    return failures


def summarize_case(matrix: MatrixSpec, result_path: Path) -> dict[str, Any]:
    rows = load_json(result_path)
    row_failures = [contract_failures(row) for row in rows]
    failures = [
        f"size {row.get('size', index)}: {failure}"
        for index, failures_for_row in enumerate(row_failures)
        for failure in failures_for_row
    ]
    prompts = sorted({str(row.get("prompt") or "") for row in rows if row.get("prompt")})
    outcomes = sorted(
        {str(row.get("expected_outcome") or "") for row in rows if row.get("expected_outcome")}
    )
    tool_parts = [
        part
        for row in rows
        for part in row.get("session_tool_parts", [])
        if isinstance(part, dict)
    ]
    return {
        "matrix": matrix.name,
        "case": result_path.parent.name,
        "status": "passed" if not failures else "failed",
        "result_path": str(result_path),
        "artifact_dir": str(result_path.parent),
        "prompts": prompts,
        "expected_outcomes": outcomes,
        "sizes": [str(row.get("size")) for row in rows],
        "terminal_markers": [row.get("terminal_marker") for row in rows],
        "contracts": {
            key: sorted({str(row.get(key)) for row in rows if row.get(key) is not None})
            for key in CONTRACT_KEYS
        },
        "tool_started": sum(int(row.get("session_event_tool_started_count") or 0) for row in rows),
        "tool_results": sum(
            int(row.get("session_event_tool_result_completed_count") or 0) for row in rows
        ),
        "projection_event_seq_max": max(
            (int(row.get("projection_event_max_seq") or 0) for row in rows),
            default=0,
        ),
        "tool_parts": tool_parts,
        "failures": failures,
    }


def discover_matrix(matrix: MatrixSpec, strict_missing: bool) -> dict[str, Any]:
    result_paths = sorted(
        path
        for path in matrix.path.glob("*/result.json")
        if path.parent.name not in {"_readiness", "_results"}
    )
    cases: list[dict[str, Any]] = []
    errors: list[str] = []
    if not matrix.path.exists():
        errors.append(f"matrix directory does not exist: {matrix.path}")
    elif strict_missing and not result_paths:
        errors.append(f"matrix directory contains no case result.json files: {matrix.path}")
    for result_path in result_paths:
        try:
            cases.append(summarize_case(matrix, result_path))
        except ValueError as exc:
            errors.append(str(exc))
    failed_cases = [case for case in cases if case["status"] != "passed"]
    status = "passed" if not errors and not failed_cases else "failed"
    return {
        "name": matrix.name,
        "path": str(matrix.path),
        "status": status,
        "case_count": len(cases),
        "passed": len(cases) - len(failed_cases),
        "failed": len(failed_cases),
        "errors": errors,
        "cases": cases,
    }


def make_report(matrices: list[MatrixSpec], strict_missing: bool) -> dict[str, Any]:
    matrix_reports = [discover_matrix(matrix, strict_missing) for matrix in matrices]
    total_cases = sum(int(matrix["case_count"]) for matrix in matrix_reports)
    failed_cases = sum(int(matrix["failed"]) for matrix in matrix_reports)
    matrix_errors = sum(len(matrix["errors"]) for matrix in matrix_reports)
    return {
        "schema": "tui_readiness_report.v1",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "status": "passed" if failed_cases == 0 and matrix_errors == 0 else "failed",
        "total_cases": total_cases,
        "passed_cases": total_cases - failed_cases,
        "failed_cases": failed_cases,
        "matrix_errors": matrix_errors,
        "matrices": matrix_reports,
    }


def markdown_table_value(values: list[Any]) -> str:
    if not values:
        return "-"
    compact = [str(value) for value in values if value not in {None, ""}]
    return ", ".join(compact) if compact else "-"


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# TUI Readiness Report",
        "",
        f"- generated_at: `{report['generated_at']}`",
        f"- status: `{report['status']}`",
        f"- cases: `{report['passed_cases']}/{report['total_cases']}` passed",
        "",
        "## Tool Turn Spine Matrix",
        "",
        "| Matrix | Case | Status | Outcome | Tools | Seq | Contracts | Notes |",
        "|---|---|---:|---|---:|---:|---|---|",
    ]
    for matrix in report["matrices"]:
        for error in matrix["errors"]:
            lines.append(
                f"| {matrix['name']} | - | failed | - | - | - | - | {error.replace('|', '/') } |"
            )
        for case in matrix["cases"]:
            contracts = []
            for key in CONTRACT_KEYS:
                states = case["contracts"].get(key) or []
                if states:
                    contracts.append(f"{key.replace('_contract', '')}:{'/'.join(states)}")
            notes = "; ".join(case["failures"]) if case["failures"] else "-"
            lines.append(
                "| {matrix} | {case} | {status} | {outcome} | {tools} | {seq} | {contracts} | {notes} |".format(
                    matrix=case["matrix"],
                    case=case["case"],
                    status=case["status"],
                    outcome=markdown_table_value(case["expected_outcomes"]),
                    tools=case["tool_results"],
                    seq=case["projection_event_seq_max"],
                    contracts=", ".join(contracts) if contracts else "-",
                    notes=notes.replace("|", "/"),
                )
            )
    lines.extend(
        [
            "",
            "## Reading",
            "",
            "- `passed` means the selected PTY run observed the requested tool turn contract for its expected outcome.",
            "- `provider-timeout` is allowed only when the tool result was observed and persisted before the provider timed out.",
            "- `projection` checks that durable event seq is contiguous and projected tool parts are anchored and current.",
            "- Any raw provider deserialization noise in the terminal fails the report.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--matrix",
        action="append",
        type=parse_matrix,
        required=True,
        help="matrix to summarize as NAME=PATH; can be repeated",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("target/tui-readiness-report"),
        help="directory for readiness.json and readiness.md",
    )
    parser.add_argument(
        "--allow-empty",
        action="store_true",
        help="do not fail when a matrix directory has no result.json files",
    )
    args = parser.parse_args()

    report = make_report(args.matrix, strict_missing=not args.allow_empty)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    json_path = args.out_dir / "readiness.json"
    markdown_path = args.out_dir / "readiness.md"
    json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    markdown_path.write_text(render_markdown(report), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())

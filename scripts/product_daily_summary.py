#!/usr/bin/env python3
"""Generate the product daily gate summary from collected live-eval artifacts."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from collections import Counter
from typing import Any

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.live_eval_report_parser import (
    derived_trajectory_metrics_from_events,
    memory_proposal_metrics_from_trace,
    report_value,
    runtime_spine_metrics_from_events,
    score_live_eval_record,
    status_value,
    status_values,
)


def read_text(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8") if path.exists() else ""


def read_json(path: pathlib.Path) -> Any:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}


def read_events(path: pathlib.Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    if not path.exists():
        return events
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return events


def capability_level(repo_root: pathlib.Path, case_id: str) -> str:
    task_file = repo_root / "evalsets" / "live_tasks" / f"{case_id}.yaml"
    if not task_file.exists():
        return "unknown"
    match = re.search(
        r"^capability_level:\s*(\d+)\s*$",
        task_file.read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    return match.group(1) if match else "unknown"


def summary_failure_owner(quality_owner: str, stderr: str, output: str) -> str:
    text = "\n".join([stderr, output]).lower()
    provider_timeout_markers = (
        "non-streaming chat timed out after",
        "chat timed out after",
        "tool-result continuation timed out after",
        "provider health step timed out after",
    )
    if any(marker in text for marker in provider_timeout_markers):
        return "environment"
    return quality_owner


def score_case(
    quality_status: str,
    eval_intent: str,
    required_status: str,
    verification_status: str,
    closeout_status: str,
    behavior_status: str,
    output_status: str,
    trajectory_status: str,
    runtime_spine_status: str,
    diff: str,
    warnings: list[str],
    failures: list[str],
) -> dict[str, Any]:
    try:
        return score_live_eval_record(
            {
                "status": "failed" if quality_status == "failed" else quality_status,
                "intent": eval_intent,
                "required": required_status,
                "verification": verification_status,
                "closeout": closeout_status,
                "behavior_assertion_status": behavior_status,
                "output_assertion_status": output_status,
                "trajectory_assertion_status": trajectory_status,
                "runtime_spine_status": runtime_spine_status,
                "diff": diff,
                "warnings": warnings,
                "failures": failures,
            }
        )
    except Exception:
        return {}


def collect_case(repo_root: pathlib.Path, run_dir: pathlib.Path, case_id: str) -> dict[str, Any]:
    case_dir = run_dir / case_id
    if not case_dir.exists():
        return {
            "id": case_id,
            "status": "missing",
            "capability_level": capability_level(repo_root, case_id),
            "reason": "no run directory",
        }

    test_status = read_text(case_dir / "test-status.txt").strip() or "unknown"
    output = read_text(case_dir / "agent-output.md")
    stderr = read_text(case_dir / "agent-stderr.log")
    events = read_events(case_dir / "agent-events.jsonl")
    diff = read_text(case_dir / "diff.patch")
    metrics = read_json(case_dir / "agent-run-metrics.json")

    quality_text = read_text(case_dir / "agent-quality-status.txt")
    quality_status = status_value(quality_text, "status", "missing")
    quality_failures = status_values(quality_text, "failure")
    quality_warnings = status_values(quality_text, "warning")
    quality_owner = status_value(quality_text, "failure_owner", "unknown")

    report_text = read_text(case_dir / "report.md")
    required_status = report_value(report_text, "required_command_status", test_status)
    closeout_status = report_value(report_text, "closeout_status", "unknown")
    verification_passed_text = report_value(report_text, "verification_passed", "unknown")
    verification_status = (
        "passed"
        if verification_passed_text == "true"
        else "failed"
        if verification_passed_text == "false"
        else "unknown"
    )
    runtime_spine_status = report_value(report_text, "runtime_spine_status", "unknown")
    behavior_status = report_value(report_text, "behavior_assertion_status", "none")
    output_status = report_value(report_text, "output_assertion_status", "none")
    trajectory_status = report_value(report_text, "trajectory_assertion_status", "none")
    eval_intent = report_value(report_text, "eval_intent", "unknown")

    score_result = score_case(
        quality_status=quality_status,
        eval_intent=eval_intent,
        required_status=required_status,
        verification_status=verification_status,
        closeout_status=closeout_status,
        behavior_status=behavior_status,
        output_status=output_status,
        trajectory_status=trajectory_status,
        runtime_spine_status=runtime_spine_status,
        diff="yes" if diff.strip() else "no",
        warnings=quality_warnings,
        failures=quality_failures,
    )

    try:
        spine_metrics = runtime_spine_metrics_from_events(events)
    except Exception:
        spine_metrics = {}
    try:
        memory_metrics = memory_proposal_metrics_from_trace(events)
    except Exception:
        memory_metrics = {}
    try:
        trajectory_metrics = derived_trajectory_metrics_from_events(events)
    except Exception:
        trajectory_metrics = {}

    return {
        "id": case_id,
        "capability_level": capability_level(repo_root, case_id),
        "status": quality_status if quality_status != "missing" else test_status,
        "test_status": test_status,
        "quality_status": quality_status,
        "failures": quality_failures,
        "warnings": quality_warnings,
        "outcome_score": score_result.get("outcome_score"),
        "process_score": score_result.get("process_score"),
        "efficiency_score": score_result.get("efficiency_score"),
        "agent_score": score_result.get("agent_score"),
        "failure_owner": summary_failure_owner(quality_owner, stderr, output),
        "closeout_status": closeout_status,
        "verification_passed": verification_passed_text,
        "required_command_status": required_status,
        "runtime_spine_phases": spine_metrics.get("phases_seen", []),
        "memory_active": memory_metrics.get("active", False),
        "memory_proposals": memory_metrics.get("proposal_count", 0),
        "tool_errors": trajectory_metrics.get("tool_errors", 0),
        "tool_round_count": trajectory_metrics.get("tool_call_count", 0),
        "changed_files": trajectory_metrics.get("changed_files", 0),
        "provider_family": metrics.get("provider_family", "unknown"),
        "provider_model": metrics.get("provider_model", "unknown"),
        "streaming_tool_mode": metrics.get("streaming_tool_mode", "unknown"),
        "termination_reason": metrics.get("termination_reason", "unknown"),
        "elapsed_secs": metrics.get("elapsed_secs"),
        "first_activity_after_start_secs": metrics.get("first_activity_after_start_secs"),
        "first_effective_action_after_start_secs": metrics.get(
            "first_effective_action_after_start_secs"
        ),
        "no_effective_progress_for_secs": metrics.get("no_effective_progress_for_secs"),
    }


def secs(value: Any) -> str:
    return f"{value:.0f}s" if isinstance(value, (int, float)) else "-"


def render_markdown(summary: dict[str, Any]) -> str:
    lines = [
        "# Product Daily Gate Summary",
        "",
        f"- Run id: `{summary['run_id']}`",
        f"- Layer: `{summary['layer']}`",
        f"- Generated: {summary['generated']}",
        f"- Pass rate: **{summary['pass_rate']}**",
        "",
        "## Results",
        "",
        f"| {'Case':<40} | {'Level':<5} | {'Status':<12} | {'Score':<6} | {'Owner':<15} | {'Time':<8} | {'Provider':<18} | {'Closeout':<12} |",
        f"|{'-' * 42}|{'-' * 7}|{'-' * 14}|{'-' * 8}|{'-' * 17}|{'-' * 10}|{'-' * 20}|{'-' * 14}|",
    ]

    for row in summary["cases"]:
        score = row.get("agent_score")
        score_str = f"{score:.0f}" if isinstance(score, (int, float)) else str(score or "-")
        provider = (row.get("provider_model") or row.get("provider_family") or "-")[:18]
        elapsed = row.get("elapsed_secs")
        time_str = f"{elapsed:.0f}s" if isinstance(elapsed, (int, float)) else "-"
        lines.append(
            f"| {row['id']:<40} | {row.get('capability_level', 'unknown'):<5} | "
            f"{row.get('status', '?'):<12} | {score_str:<6} | "
            f"{(row.get('failure_owner') or '-'):<15} | {time_str:<8} | "
            f"{provider:<18} | {(row.get('closeout_status') or '-'):<12} |"
        )

    lines.extend(
        [
            "",
            "## Slow Tail Metrics",
            "",
            f"| {'Case':<40} | {'Termination':<28} | {'First activity':<14} | {'First diff':<12} | {'No effective progress':<22} | {'Tool rounds':<11} |",
            f"|{'-' * 42}|{'-' * 30}|{'-' * 16}|{'-' * 14}|{'-' * 24}|{'-' * 13}|",
        ]
    )
    for row in summary["cases"]:
        lines.append(
            f"| {row['id']:<40} | {str(row.get('termination_reason') or '-'):<28} | "
            f"{secs(row.get('first_activity_after_start_secs')):<14} | "
            f"{secs(row.get('first_effective_action_after_start_secs')):<12} | "
            f"{secs(row.get('no_effective_progress_for_secs')):<22} | "
            f"{str(row.get('tool_round_count', '-')):<11} |"
        )

    owner_counts: Counter[str] = Counter()
    for row in summary["cases"]:
        owner = row.get("failure_owner", "unknown") or "unknown"
        if owner != "none" and row.get("status") not in ("ok", "missing"):
            owner_counts[owner] += 1

    lines.extend(["", "## Failure Owners", ""])
    if owner_counts:
        for owner, count in owner_counts.most_common():
            lines.append(f"- `{owner}`: {count}")
    else:
        lines.append("- No failures")

    phase_counts: Counter[str] = Counter()
    for row in summary["cases"]:
        phase_counts.update(row.get("runtime_spine_phases", []))

    lines.extend(["", "## Runtime Spine Coverage", ""])
    if phase_counts:
        for phase, count in phase_counts.most_common():
            lines.append(f"- `{phase}`: {count}/{summary['total']} cases")
    else:
        lines.append("- No spine data")

    lines.extend(["", "## Next Steps", ""])
    if summary["failed"] > 0:
        lines.append(
            f"- {summary['failed']} case(s) failed. Check failure owners above for triage."
        )
        for row in summary["cases"]:
            if row.get("status") not in ("ok", "missing"):
                lines.append(
                    f"  - `{row['id']}`: owner=`{row.get('failure_owner', 'unknown')}`"
                )
    else:
        lines.append("- All cases passed. No immediate action required.")
    lines.append("")
    return "\n".join(lines)


def build_summary(repo_root: pathlib.Path, run_dir: pathlib.Path, cases: list[str], run_id: str, layer: str) -> dict[str, Any]:
    results = [collect_case(repo_root, run_dir, case_id) for case_id in cases]
    passed = sum(1 for row in results if row.get("status") == "ok")
    failed = sum(1 for row in results if row.get("status") in ("failed", "not_verified", "error"))
    missing = sum(1 for row in results if row.get("status") == "missing")
    total = len(results)
    return {
        "run_id": run_id,
        "layer": layer,
        "generated": dt.datetime.now().isoformat(),
        "total": total,
        "passed": passed,
        "failed": failed,
        "missing": missing,
        "pass_rate": f"{passed}/{total}" if total > 0 else "0/0",
        "cases": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", required=True, type=pathlib.Path)
    parser.add_argument("--cases", required=True, help="space-separated case ids")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--layer", required=True)
    parser.add_argument("--report", required=True, type=pathlib.Path)
    parser.add_argument("--json", required=True, type=pathlib.Path)
    parser.add_argument("--repo-root", default=".", type=pathlib.Path)
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    run_dir = args.run_dir
    cases = [case for case in args.cases.split() if case]
    summary = build_summary(repo_root, run_dir, cases, args.run_id, args.layer)

    args.json.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    args.report.write_text(render_markdown(summary), encoding="utf-8")
    print(f"Report: {args.report}")
    print(f"JSON:   {args.json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

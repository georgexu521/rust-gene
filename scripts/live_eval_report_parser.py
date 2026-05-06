import pathlib
import re


def read(path):
    path = pathlib.Path(path)
    return path.read_text(encoding="utf-8") if path.exists() else ""


def status_value(text, key, default="missing"):
    match = re.search(rf"^{re.escape(key)}=(.+)$", text, re.MULTILINE)
    return match.group(1).strip() if match else default


def report_value(text, key, default="missing"):
    match = re.search(rf"^{re.escape(key)}:\s*(.+)$", text, re.MULTILINE)
    return match.group(1).strip() if match else default


def report_values(text, key):
    return [
        match.group(1).strip()
        for match in re.finditer(rf"^{re.escape(key)}:\s*(.+)$", text, re.MULTILINE)
    ]


def status_values(text, key):
    return [
        match.group(1).strip()
        for match in re.finditer(rf"^{re.escape(key)}=(.+)$", text, re.MULTILINE)
    ]


def has_warning(text, warning):
    return bool(re.search(rf"^warning={re.escape(warning)}$", text, re.MULTILINE))


def unique_items(items):
    seen = set()
    result = []
    for item in items:
        if item and item not in seen:
            seen.add(item)
            result.append(item)
    return result


def report_rows(run_dir):
    run_dir = pathlib.Path(run_dir)
    rows = []
    for report in sorted(run_dir.glob("*/report.md")):
        task_dir = report.parent
        task_id = task_dir.name
        report_text = read(report)
        quality_text = read(task_dir / "agent-quality-status.txt")
        test_status = read(task_dir / "test-status.txt").strip() or "missing"
        diff_stat = read(task_dir / "diff-stat.txt").strip()
        agent_output = read(task_dir / "agent-output.md")
        plan_file = task_dir / "minimax-plan.md"
        plan_lint = task_dir / "plan-lint.txt"
        api_response = task_dir / "api-response.json"
        agent_events = task_dir / "agent-events.jsonl"
        quality_status = status_value(quality_text, "status", "missing")
        failure_owner = status_value(
            quality_text,
            "failure_owner",
            report_value(report_text, "failure_owner", "missing"),
        )
        eval_intent = report_value(report_text, "eval_intent", "missing")
        closeout = report_value(report_text, "closeout_status", "missing")
        adaptive_triggers = report_value(report_text, "adaptive_triggers", "none")
        first_write = report_value(report_text, "first_write_tool_index", "missing")
        required = report_value(report_text, "required_command_status", test_status)
        if plan_file.exists():
            plan_quality = status_value(read(plan_lint), "status", "missing")
        elif api_response.exists():
            plan_quality = "api_response"
        else:
            plan_quality = "none"
        if agent_events.exists():
            tool_boundary = "agent-run"
        elif plan_file.exists() or api_response.exists():
            tool_boundary = "plan-only"
        else:
            tool_boundary = "collect-only"
        if closeout == "passed" and required == "ok":
            verification_status = "passed"
        elif quality_status == "failed" or test_status == "failed":
            verification_status = "failed"
        else:
            verification_status = "unknown"
        run_status = (
            "passed"
            if quality_status in {"ok", "missing"}
            and test_status in {"ok", "skipped", "missing"}
            else "failed"
        )
        warnings = []
        output_warning_markers = {
            "action_checkpoint_no_patch": "Stopped action checkpoint without patch synthesis",
            "action_checkpoint_invalid_tools": "Stopped action checkpoint after repeated invalid tool requests",
            "patch_synthesis_no_change": "Patch synthesis did not produce a file change",
        }
        for warning in (
            "no_code_diff",
            "audit_no_code_diff",
            "current_head_no_fixture_already_satisfied",
            "tool_errors_seen",
            "action_checkpoint_no_patch",
            "action_checkpoint_invalid_tools",
            "patch_synthesis_no_change",
        ):
            if (
                has_warning(quality_text, warning)
                or warning in report_values(report_text, "warning")
                or output_warning_markers.get(warning, "\0") in agent_output
            ):
                warnings.append(warning)
        failures = unique_items(
            status_values(quality_text, "failure")
            + [
                warning
                for warning in report_values(report_text, "warning")
                if warning not in warnings
            ]
        )
        rows.append(
            {
                "task": task_id,
                "status": run_status,
                "intent": eval_intent,
                "owner": failure_owner,
                "required": required,
                "plan": plan_quality,
                "boundary": tool_boundary,
                "verification": verification_status,
                "closeout": closeout,
                "triggers": adaptive_triggers,
                "first_write": first_write,
                "diff": "yes" if diff_stat else "no",
                "warnings": ",".join(warnings) if warnings else "none",
                "failures": failures,
                "has_output": bool(agent_output.strip()),
            }
        )
    return rows

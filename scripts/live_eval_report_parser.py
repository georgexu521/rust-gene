import pathlib
import re
import json


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


def jsonl_events(path):
    events = []
    path = pathlib.Path(path)
    if not path.exists():
        return events
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            events.append(json.loads(line))
        except Exception:
            continue
    return events


def latest_trace(events):
    return next(
        (event for event in reversed(events) if event.get("event") == "trace_summary"),
        {},
    )


def trace_events(events):
    return (latest_trace(events).get("trace") or {}).get("events") or []


def trace_event_types(events):
    return latest_trace(events).get("event_types") or []


def bool_text(value):
    return "true" if value else "false"


def parse_boolish(value):
    return str(value).strip().lower() in {"1", "true", "yes", "on"}


def int_text(value, default=0):
    try:
        return str(int(value))
    except Exception:
        return str(default)


def specialty_metrics(task_dir, report_text):
    events = jsonl_events(task_dir / "agent-events.jsonl")
    event_types = trace_event_types(events)
    trace_items = trace_events(events)
    tool_starts = [
        event for event in events if event.get("event") == "tool_execution_start"
    ]

    retrieval_events = [
        event for event in trace_items if event.get("type") == "retrieval_context_built"
    ]
    memory_retrievals = [
        event
        for event in retrieval_events
        if any(str(source) == "Memory" for source in event.get("sources") or [])
    ]
    memory_tools = [
        event
        for event in tool_starts
        if str(event.get("name", "")).startswith("memory")
    ]
    skill_tools = [
        event
        for event in tool_starts
        if str(event.get("name", "")).startswith("skill")
        or str(event.get("name", "")) == "skill_manage"
    ]
    learning_adjusted = any(
        event.get("type") == "workflow_learning_adjusted" for event in trace_items
    )
    plan_reweighted = any(
        event.get("type") == "workflow_plan_progress" and event.get("reweighted") is True
        for event in trace_items
    )
    memory_sync_events = event_types.count("memory.sync")
    memory_recalled_items = sum(int(event.get("items") or 0) for event in memory_retrievals)
    memory_conflicts = sum(int(event.get("conflicts") or 0) for event in memory_retrievals)

    report_memory_active = report_value(report_text, "memory_active", "")
    memory_active = (
        parse_boolish(report_memory_active)
        if report_memory_active
        else bool(memory_sync_events or memory_tools or memory_retrievals)
    )
    report_skill_active = report_value(report_text, "skill_active", "")
    skill_active = (
        parse_boolish(report_skill_active)
        if report_skill_active
        else bool(skill_tools or "skill" in " ".join(event_types))
    )
    memory_changed_plan = parse_boolish(
        report_value(report_text, "memory_changed_plan", "")
    ) or learning_adjusted or plan_reweighted
    skill_promotion_evidence = parse_boolish(
        report_value(report_text, "skill_promotion_evidence", "")
    ) or bool(skill_tools)

    memory_summary = "active={active}, recalled={recalled}, conflicts={conflicts}, changed_plan={changed}".format(
        active=bool_text(memory_active),
        recalled=int_text(report_value(report_text, "memory_recalled_items", memory_recalled_items)),
        conflicts=int_text(report_value(report_text, "memory_conflicts", memory_conflicts)),
        changed=bool_text(memory_changed_plan),
    )
    skill_summary = "active={active}, tool_calls={tool_calls}, usage_events={usage}, promotion={promotion}".format(
        active=bool_text(skill_active),
        tool_calls=int_text(report_value(report_text, "skill_tool_calls", len(skill_tools))),
        usage=int_text(report_value(report_text, "skill_usage_events", 0)),
        promotion=bool_text(skill_promotion_evidence),
    )

    return {
        "memory_active": bool_text(memory_active),
        "memory_recalled_items": int_text(
            report_value(report_text, "memory_recalled_items", memory_recalled_items)
        ),
        "memory_conflicts": int_text(
            report_value(report_text, "memory_conflicts", memory_conflicts)
        ),
        "memory_changed_plan": bool_text(memory_changed_plan),
        "memory": memory_summary,
        "skill_active": bool_text(skill_active),
        "skill_tool_calls": int_text(report_value(report_text, "skill_tool_calls", len(skill_tools))),
        "skill_usage_events": int_text(report_value(report_text, "skill_usage_events", 0)),
        "skill_promotion_evidence": bool_text(skill_promotion_evidence),
        "skill": skill_summary,
    }


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
        runtime_diet = report_value(report_text, "runtime_diet", "missing")
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
        specialty = specialty_metrics(task_dir, report_text)
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
                "runtime_diet": runtime_diet,
                "triggers": adaptive_triggers,
                "first_write": first_write,
                "diff": "yes" if diff_stat else "no",
                "warnings": ",".join(warnings) if warnings else "none",
                "failures": failures,
                "has_output": bool(agent_output.strip()),
                **specialty,
            }
        )
    return rows

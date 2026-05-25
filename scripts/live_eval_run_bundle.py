#!/usr/bin/env python3
import argparse
import datetime as dt
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.live_eval_report_parser import jsonl_events, read, report_rows, report_value, token


SECRET_PATTERNS = [
    re.compile(r"(?i)(api[_-]?key|token|secret|password)(['\" ]*[:=]['\" ]*)([^'\"\s,}]+)"),
    re.compile(r"(?i)bearer\s+[a-z0-9._\-]{12,}"),
    re.compile(r"\bsk-[a-zA-Z0-9_\-]{12,}\b"),
    re.compile(r"\b[a-zA-Z0-9_\-]{32,}\.[a-zA-Z0-9_\-]{16,}\.[a-zA-Z0-9_\-]{16,}\b"),
]


def redact_text(value):
    text = str(value)
    for pattern in SECRET_PATTERNS:
        if pattern.pattern.startswith("(?i)(api"):
            text = pattern.sub(lambda match: f"{match.group(1)}{match.group(2)}[REDACTED]", text)
        else:
            text = pattern.sub("[REDACTED]", text)
    home = str(pathlib.Path.home())
    if home and home in text:
        text = text.replace(home, "~")
    return text


def redact_value(value):
    if isinstance(value, dict):
        redacted = {}
        for key, item in value.items():
            key_text = str(key)
            if token(key_text) in {"api_key", "apikey", "token", "secret", "password", "authorization"}:
                redacted[key_text] = "[REDACTED]"
            else:
                redacted[key_text] = redact_value(item)
        return redacted
    if isinstance(value, list):
        return [redact_value(item) for item in value]
    if isinstance(value, str):
        return redact_text(value)
    return value


def bounded_summary(value, max_chars=500):
    text = redact_text(value)
    return text if len(text) <= max_chars else text[: max_chars - 1] + "..."


def latest_trace(events):
    return next((event for event in reversed(events) if event.get("event") == "trace_summary"), {})


def trace_items(events):
    return (latest_trace(events).get("trace") or {}).get("events") or []


def load_row(task_dir):
    rows = report_rows(task_dir.parent)
    return next((row for row in rows if row["task"] == task_dir.name), {})


def write_json(path, data):
    path.write_text(json.dumps(redact_value(data), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def write_jsonl(path, rows):
    with path.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(redact_value(row), ensure_ascii=False, sort_keys=True) + "\n")


def build_step_rows(events):
    trace = trace_items(events)
    agent_loop_events = [
        event for event in trace if token(event.get("type", "")) == "agent_loop_step_evaluated"
    ]
    if agent_loop_events:
        rows = []
        for index, event in enumerate(agent_loop_events, start=1):
            rows.append(
                {
                    "schema": "agent_run_step.v1",
                    "step": index,
                    "stage_before": event.get("stage_before", "missing"),
                    "stage_after": event.get("stage_after", "missing"),
                    "selected_action": event.get("selected_action", event.get("action", "missing")),
                    "stop_reason": event.get("stop_reason", "missing"),
                    "observation": bounded_summary(event.get("observation", event.get("summary", ""))),
                    "raw_event_type": event.get("type", "agent_loop_step_evaluated"),
                }
            )
        return rows

    tool_starts = [event for event in events if token(event.get("event", "")) == "tool_execution_start"]
    tool_done = {
        str(event.get("id", "")): event
        for event in events
        if token(event.get("event", "")) == "tool_execution_complete"
    }
    rows = []
    for index, event in enumerate(tool_starts, start=1):
        done = tool_done.get(str(event.get("id", "")), {})
        rows.append(
            {
                "schema": "agent_run_step.v1",
                "step": index,
                "stage_before": "missing",
                "stage_after": "missing",
                "selected_action": event.get("name", "missing"),
                "permission_result": "missing",
                "tool_name": event.get("name", "missing"),
                "tool_result_status": "error"
                if "result: error" in str(done.get("result_preview", "")).lower()
                else "ok",
                "observation": bounded_summary(done.get("result_preview", "")),
                "raw_event_type": "tool_execution_start",
            }
        )
    return rows


def build_event_rows(events):
    rows = []
    for index, event in enumerate(events, start=1):
        event_name = event.get("event", "unknown")
        if event_name == "trace_summary":
            trace = event.get("trace") or {}
            rows.append(
                {
                    "schema": "agent_run_event.v1",
                    "index": index,
                    "event": event_name,
                    "trace_id": event.get("trace_id", "missing"),
                    "status": event.get("status", "missing"),
                    "turn_index": event.get("turn_index", "missing"),
                    "duration_ms": event.get("duration_ms", "missing"),
                    "event_count": len(trace.get("events", [])),
                    "event_types": event.get("event_types", []),
                }
            )
            continue
        rows.append(
            {
                "schema": "agent_run_event.v1",
                "index": index,
                "event": event_name,
                "id": event.get("id", "missing"),
                "name": event.get("name", event.get("tool_name", "missing")),
                "summary": bounded_summary(
                    event.get("result_preview")
                    or event.get("progress")
                    or event.get("message")
                    or event.get("prompt")
                    or ""
                ),
                "metadata": event.get("metadata", {}),
            }
        )
    return rows


def final_report(task_id, run_id, row, report_text):
    validation = row.get("required", report_value(report_text, "required_command_status", "missing"))
    lines = [
        f"# Agent Run Bundle: {task_id}",
        "",
        f"- Run id: `{run_id}`",
        f"- Final status: `{row.get('status', 'missing')}`",
        f"- Terminal status: `{row.get('completion_contract_terminal_status', row.get('stop_terminal_status', 'missing'))}`",
        f"- Stop reason: `{row.get('stop_reason', 'missing')}`",
        f"- Required command status: `{validation}`",
        f"- Verification status: `{row.get('verification', 'missing')}`",
        f"- Closeout status: `{row.get('closeout', 'missing')}`",
        f"- Runtime spine: `{row.get('runtime_spine', 'missing')}`",
        f"- Outcome score: `{row.get('outcome_score', 'missing')}`",
        f"- Process score: `{row.get('process_score', 'missing')}`",
        f"- Efficiency score: `{row.get('efficiency_score', 'missing')}`",
        f"- Agent score: `{row.get('agent_score', 'missing')}`",
        f"- Score penalties: `{row.get('score_penalties', 'none')}`",
        "",
        "## Key Metrics",
        "",
        f"- Tool calls: `{row.get('tool_call_count', row.get('tool_executions', '0'))}`",
        f"- Failed actions: `{row.get('failed_action_count', '0')}`",
        f"- Repeated actions: `{row.get('repeated_action_count', '0')}`",
        f"- Premature edits: `{row.get('premature_edit_count', '0')}`",
        f"- Scope drift count: `{row.get('scope_drift_count', '0')}`",
        f"- Invalid action count: `{row.get('invalid_action_count', '0')}`",
        "",
        "## Artifacts",
        "",
        "- `task.json`",
        "- `steps.jsonl`",
        "- `events.jsonl`",
    ]
    return "\n".join(lines).rstrip() + "\n"


def main():
    parser = argparse.ArgumentParser(description="Export a redacted live-eval run bundle.")
    parser.add_argument("task_dir", help="Live-eval task report directory")
    parser.add_argument("--output-dir", default="", help="Output directory, default: <task_dir>/run-bundle")
    parser.add_argument("--run-id", default="")
    args = parser.parse_args()

    task_dir = pathlib.Path(args.task_dir)
    output_dir = pathlib.Path(args.output_dir) if args.output_dir else task_dir / "run-bundle"
    output_dir.mkdir(parents=True, exist_ok=True)

    events = jsonl_events(task_dir / "agent-events.jsonl")
    report_text = read(task_dir / "report.md")
    row = load_row(task_dir)
    run_id = args.run_id or task_dir.parent.name.removeprefix("live-")
    task_id = task_dir.name

    task_json = {
        "schema": "agent_run_bundle.v1",
        "task_id": task_id,
        "run_id": run_id,
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "goal_summary": bounded_summary(report_value(report_text, "eval_intent", task_id)),
        "mode": row.get("boundary", "missing"),
        "final_status": row.get("status", "missing"),
        "terminal_status": row.get("completion_contract_terminal_status", row.get("stop_terminal_status", "missing")),
        "stop_reason": row.get("stop_reason", "missing"),
        "modified_files": row.get("diff_files_changed", "missing"),
        "validation_status": row.get("required", "missing"),
        "verification_status": row.get("verification", "missing"),
        "closeout_status": row.get("closeout", "missing"),
        "scores": {
            "outcome": row.get("outcome_score", "missing"),
            "process": row.get("process_score", "missing"),
            "efficiency": row.get("efficiency_score", "missing"),
            "agent": row.get("agent_score", "missing"),
            "penalties": row.get("score_penalties", "none"),
        },
        "artifacts": {
            "source_report": str(task_dir / "report.md"),
            "source_events": str(task_dir / "agent-events.jsonl"),
            "steps": str(output_dir / "steps.jsonl"),
            "events": str(output_dir / "events.jsonl"),
            "final_report": str(output_dir / "final_report.md"),
        },
    }

    write_json(output_dir / "task.json", task_json)
    write_jsonl(output_dir / "steps.jsonl", build_step_rows(events))
    write_jsonl(output_dir / "events.jsonl", build_event_rows(events))
    (output_dir / "final_report.md").write_text(
        redact_text(final_report(task_id, run_id, row, report_text)),
        encoding="utf-8",
    )
    print(output_dir)


if __name__ == "__main__":
    main()

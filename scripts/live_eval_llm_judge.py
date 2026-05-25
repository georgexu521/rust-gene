#!/usr/bin/env python3
import argparse
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.live_eval_run_bundle import bounded_summary, redact_value
from scripts.live_eval_report_parser import read


def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def skipped(reason):
    return {
        "schema": "agent_eval_judge.v1",
        "status": "skipped",
        "reason": reason,
        "outcome": None,
        "process": None,
        "tool_use": None,
        "risk": None,
        "user_burden": None,
        "goal_drift": None,
        "premature_edit": None,
        "findings": [],
    }


def main():
    parser = argparse.ArgumentParser(description="Run a gated redacted eval judge hook.")
    parser.add_argument("task_dir", help="Live-eval task report directory")
    parser.add_argument("--output", default="")
    args = parser.parse_args()

    task_dir = pathlib.Path(args.task_dir)
    output = pathlib.Path(args.output) if args.output else task_dir / "judge.json"

    if os.environ.get("PRIORITY_AGENT_EVAL_LLM_JUDGE") != "1":
        return 0

    command = os.environ.get("PRIORITY_AGENT_EVAL_JUDGE_COMMAND", "").strip()
    if not command:
        write_json(output, skipped("PRIORITY_AGENT_EVAL_JUDGE_COMMAND is not set"))
        print(output)
        return 0

    bundle_dir = task_dir / "run-bundle"
    payload = {
        "schema": "agent_eval_judge_input.v1",
        "task": json.loads(read(bundle_dir / "task.json") or "{}"),
        "steps": [
            json.loads(line)
            for line in read(bundle_dir / "steps.jsonl").splitlines()
            if line.strip()
        ][:40],
        "final_report": bounded_summary(read(bundle_dir / "final_report.md"), 4000),
        "agent_output": bounded_summary(read(task_dir / "agent-output.md"), 4000),
    }
    payload = redact_value(payload)

    proc = subprocess.run(
        command,
        input=json.dumps(payload, ensure_ascii=False),
        text=True,
        shell=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=int(os.environ.get("PRIORITY_AGENT_EVAL_JUDGE_TIMEOUT_SECS", "120")),
    )
    if proc.returncode != 0:
        write_json(output, skipped(f"judge command failed with exit status {proc.returncode}"))
        print(output)
        return 0

    try:
        result = json.loads(proc.stdout)
    except Exception:
        write_json(output, skipped("judge command did not return JSON"))
        print(output)
        return 0

    result.setdefault("schema", "agent_eval_judge.v1")
    result.setdefault("status", "completed")
    write_json(output, redact_value(result))
    print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())

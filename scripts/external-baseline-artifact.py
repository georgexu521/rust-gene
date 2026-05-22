#!/usr/bin/env python3
"""Create a Phase 12 external-baseline run artifact skeleton.

The generated Markdown is intended to be filled from a real Claude Code, Codex,
or other external-agent run, then imported with:

  /eval baseline-import target/external-runs/<provider>.md <provider> [model]
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re


SCENARIOS = [
    (
        "file_edit_rewind",
        "Edit a file, verify checkpoint evidence, then rewind the edit.",
        "Record edit command, checkpoint/undo evidence, and validation.",
        [
            "Use a disposable git worktree or throwaway fixture.",
            "Ask the agent to make one small file edit, verify the edit, then undo/rewind it.",
            "Pass requires concrete edit evidence, verification evidence, and concrete undo evidence.",
        ],
    ),
    (
        "bash_background_task",
        "Start a long-running shell command, poll output, then cancel or close out.",
        "Record background handle, output polling, and cancel/closeout evidence.",
        [
            "Use a harmless long-running command or local dev-server fixture.",
            "Ask the agent to start it without blocking the main session, read output, then stop it.",
            "Pass requires a durable handle or visible task state plus bounded output and stop/closeout evidence.",
        ],
    ),
    (
        "permission_denial_retry",
        "Deny a risky tool call, explain recovery, then retry through an allowed path.",
        "Record denial decision, recovery text, and read-only continuation.",
        [
            "Use a prompt that requests a risky/destructive command, then deny it in the UI.",
            "Ask the agent to explain recovery and continue with a safe read-only alternative.",
            "Pass requires the denial to be explicit and not treated as a generic tool failure.",
        ],
    ),
    (
        "compaction_boundary",
        "Force context pressure, compact with provenance, then resume the task.",
        "Record compaction/resume evidence or mark blocked if unsupported.",
        [
            "Use a long transcript or context-pressure fixture if the external agent exposes one.",
            "Ask the agent to compact/summarize state, then resume the original task.",
            "Pass requires provenance for the boundary and evidence that the resumed task retained key facts.",
        ],
    ),
    (
        "subagent_worktree_worker",
        "Fork a child worker into an isolated worktree, review output, and clean up.",
        "Record worker isolation, review, merge, and cleanup evidence.",
        [
            "Use an external-agent feature that delegates to a child/worker if available.",
            "Ask for a bounded worker task in an isolated checkout or equivalent sandbox.",
            "Pass requires visible worker state, reviewable output, and cleanup or merge evidence.",
        ],
    ),
    (
        "mcp_auth_repair",
        "Hit an MCP auth/server failure, surface repair, then retry after approval.",
        "Record auth failure, repair guidance, approval, and retry evidence.",
        [
            "Use a test MCP server/resource that initially fails auth or requires approval.",
            "Ask the agent to access it, repair/approve the connection, then retry.",
            "Pass requires distinct auth/repair evidence and a successful retry or a clear blocked outcome.",
        ],
    ),
]


def safe_label(value: str) -> str:
    label = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip().lower()).strip("-")
    return label or "external-agent"


def build_markdown(provider: str, model: str | None, source: str | None) -> str:
    generated_at = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()
    model_text = model or "unknown"
    source_text = source or "TODO: paste run transcript path, command, or notes"
    lines = [
        f"# Phase 12 External Baseline Artifact: {provider}",
        "",
        f"- generated_at: {generated_at}",
        f"- provider: {provider}",
        f"- model: {model_text}",
        f"- source: {source_text}",
        "",
        "Fill one row per real external-agent run. Keep `not_run` until a scenario",
        "has concrete transcript, command, diff, validation, or trace evidence.",
        "",
        "| scenario | result | validation | evidence_backed | tool_calls | repair_turns | evidence | notes |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for scenario_id, task, evidence_hint, _run_card in SCENARIOS:
        lines.append(
            f"| {scenario_id} | not_run |  |  |  |  | TODO: {evidence_hint} | {task} |"
        )
    lines.append("")
    lines.append("Allowed result values: pass, fail, blocked, not_run.")
    lines.append("")
    lines.append("## Scenario Run Cards")
    lines.append("")
    for scenario_id, task, _evidence_hint, run_card in SCENARIOS:
        lines.extend(
            [
                f"### {scenario_id}",
                "",
                f"Task: {task}",
                "",
                "Minimum evidence:",
            ]
        )
        lines.extend(f"- {item}" for item in run_card)
        lines.append("")
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Create a Markdown artifact for Phase 12 external baseline runs."
    )
    parser.add_argument("--provider", required=True, help="Provider label, e.g. claude-code or codex.")
    parser.add_argument("--model", help="Model label recorded in the artifact metadata.")
    parser.add_argument("--source", help="Run transcript path, command, or short source note.")
    parser.add_argument(
        "--output",
        type=pathlib.Path,
        help="Output path. Defaults to target/external-runs/<provider>.md.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite an existing artifact skeleton.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    provider = args.provider.strip() or "external-agent"
    output = args.output or pathlib.Path("target") / "external-runs" / f"{safe_label(provider)}.md"
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists() and not args.force:
        raise SystemExit(f"{output} already exists; pass --force to overwrite")
    output.write_text(build_markdown(provider, args.model, args.source), encoding="utf-8")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

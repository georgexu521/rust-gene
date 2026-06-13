#!/usr/bin/env python3
"""Run a real PTY smoke against the Priority Agent TUI.

This script intentionally checks observable terminal evidence instead of
claiming a full visual pass. It is useful for dogfooding startup, provider
turns, and tool-turn attempts in a real terminal-sized PTY.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import time

try:
    import pexpect
except ImportError as exc:  # pragma: no cover - environment guard
    raise SystemExit("pexpect is required: python3 -m pip install pexpect") from exc


ANSI_RE = re.compile(
    r"\x1b(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))"
)

PROMPTS = {
    "startup": "",
    "provider-ok": "你好，请只回复 OK，不要调用工具。",
    "tool-pwd": "请使用 bash 工具运行 pwd，然后只用一行中文告诉我命令输出。",
    "tool-fail": "请使用 bash 工具运行 sh -lc 'exit 7'，然后只用一行中文说明命令失败。",
    "tool-long": "请使用 bash 工具运行 python3 -c 'print(\"x\" * 5000)'，然后只用一行中文说明输出很长。",
    "tool-sleep": "请使用 bash 工具运行 sleep 20，然后只用一行中文说明命令完成。",
    "tool-invalid-args": "请调用 bash 工具但故意缺少 command 参数，然后说明参数错误。",
    "tool-multi": "请连续使用两个 bash 工具：先运行 pwd，再运行 echo multi-tool-ok，然后总结结果。",
    "tool-partial": "请连续使用两个 bash 工具：先运行 echo partial-ok，再运行 sh -lc 'echo partial-fail >&2; exit 9'，然后总结哪些成功哪些失败。",
    "tool-malformed": "请用一个需要 provider 修复的 bash 工具调用运行 echo malformed-repaired，然后总结结果。",
}

TOOL_PROMPTS = {
    "tool-pwd",
    "tool-fail",
    "tool-long",
    "tool-sleep",
    "tool-invalid-args",
    "tool-multi",
    "tool-partial",
    "tool-malformed",
}


def parse_size(raw: str) -> tuple[int, int]:
    try:
        cols, rows = raw.lower().split("x", 1)
        return int(cols), int(rows)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("size must look like 120x35") from exc


def strip_ansi(raw: str) -> str:
    text = ANSI_RE.sub("", raw).replace("\r", "\n")
    lines: list[str] = []
    for line in text.splitlines():
        clean = "".join(ch for ch in line if ch == "\t" or ch == " " or ch.isprintable())
        clean = clean.rstrip()
        if clean.strip():
            lines.append(clean)
    return "\n".join(lines[-400:]) + ("\n" if lines else "")


def ensure_binary(repo: Path, binary: str, build: bool) -> None:
    if Path(binary).is_absolute():
        exists = Path(binary).is_file()
    else:
        exists = (repo / binary).is_file()
    if exists:
        return
    if not build:
        raise SystemExit(f"TUI binary not found: {binary}. Run cargo build -q first.")
    subprocess.run(["cargo", "build", "-q"], cwd=repo, check=True)


def default_session_db_path() -> Path:
    configured = os.environ.get("PRIORITY_AGENT_SESSION_DB")
    if configured:
        return Path(configured).expanduser()
    return (
        Path.home()
        / "Library"
        / "Application Support"
        / "priority-agent"
        / "sessions.db"
    )


def ordered_subsequence(haystack: list[str], needle: list[str]) -> bool:
    cursor = 0
    for item in haystack:
        if cursor < len(needle) and item == needle[cursor]:
            cursor += 1
    return cursor == len(needle)


def latest_session_for_prompt(db_path: Path, prompt: str) -> str | None:
    if not db_path.exists():
        return None
    with sqlite3.connect(db_path) as conn:
        row = conn.execute(
            """
            SELECT session_id
            FROM messages
            WHERE role = 'user' AND content = ?
            ORDER BY id DESC
            LIMIT 1
            """,
            (prompt,),
        ).fetchone()
        if row:
            return str(row[0])
        row = conn.execute(
            "SELECT id FROM sessions WHERE title = ? ORDER BY updated_at DESC LIMIT 1",
            (prompt,),
        ).fetchone()
    return str(row[0]) if row else None


def session_event_types(db_path: Path, session_id: str) -> list[str]:
    with sqlite3.connect(db_path) as conn:
        rows = conn.execute(
            "SELECT event_type FROM session_events WHERE session_id = ? ORDER BY seq ASC",
            (session_id,),
        ).fetchall()
    return [str(row[0]) for row in rows]


def session_runtime_diagnostics(db_path: Path, session_id: str) -> list[dict[str, object]]:
    with sqlite3.connect(db_path) as conn:
        rows = conn.execute(
            """
            SELECT payload
            FROM session_events
            WHERE session_id = ? AND event_type = 'runtime_diagnostic'
            ORDER BY seq ASC
            """,
            (session_id,),
        ).fetchall()
    diagnostics: list[dict[str, object]] = []
    for (payload,) in rows:
        try:
            value = json.loads(str(payload))
        except json.JSONDecodeError:
            continue
        diagnostic = value.get("diagnostic") if isinstance(value, dict) else None
        if isinstance(diagnostic, dict):
            diagnostics.append(diagnostic)
    return diagnostics


def session_message_roles(db_path: Path, session_id: str) -> list[str]:
    with sqlite3.connect(db_path) as conn:
        rows = conn.execute(
            "SELECT role FROM messages WHERE session_id = ? ORDER BY id ASC",
            (session_id,),
        ).fetchall()
    return [str(row[0]) for row in rows]


def session_part_rows(db_path: Path, session_id: str) -> list[dict[str, object]]:
    with sqlite3.connect(db_path) as conn:
        rows = conn.execute(
            """
            SELECT kind, tool_name, status
            FROM session_parts
            WHERE session_id = ? AND kind = 'tool'
            ORDER BY part_index ASC, id ASC
            """,
            (session_id,),
        ).fetchall()
    return [
        {"kind": str(kind), "tool_name": tool_name, "status": status}
        for kind, tool_name, status in rows
    ]


def count_events(event_types: list[str], event_type: str) -> int:
    return sum(1 for item in event_types if item == event_type)


def assert_tool_turn_events(
    *,
    db_path: Path,
    prompt: str,
    expected_outcome: str,
    expected_tool_start_count: int | None,
    expected_tool_result_count: int | None,
) -> dict[str, object]:
    session_id = latest_session_for_prompt(db_path, prompt)
    if not session_id:
        return {
            "session_event_contract": "failed",
            "session_event_error": f"no session found for prompt title: {prompt}",
        }
    event_types = session_event_types(db_path, session_id)
    if expected_outcome == "interrupted":
        required = ["tool_started"]
        forbidden = ["tool_results_ready_for_model", "assistant_text_completed", "step_ended"]
    elif expected_outcome == "provider-timeout":
        required = [
            "tool_started",
            "tool_result_completed",
            "tool_results_ready_for_model",
        ]
        forbidden = ["assistant_text_completed", "step_ended"]
    else:
        forbidden = []
        required = [
            "tool_started",
            "tool_result_completed",
            "tool_results_ready_for_model",
            "assistant_text_completed",
            "step_ended",
        ]
    ok = ordered_subsequence(event_types, required)
    forbidden_seen = [event for event in forbidden if event in event_types]
    ok = ok and not forbidden_seen
    tool_start_count = count_events(event_types, "tool_started")
    tool_result_count = count_events(event_types, "tool_result_completed")
    count_errors: list[str] = []
    if (
        expected_tool_start_count is not None
        and tool_start_count != expected_tool_start_count
    ):
        count_errors.append(
            f"expected {expected_tool_start_count} tool_started, got {tool_start_count}"
        )
    if (
        expected_tool_result_count is not None
        and tool_result_count != expected_tool_result_count
    ):
        count_errors.append(
            f"expected {expected_tool_result_count} tool_result_completed, got {tool_result_count}"
        )
    ok = ok and not count_errors
    return {
        "session_id": session_id,
        "session_event_contract": "passed" if ok else "failed",
        "session_event_required": required,
        "session_event_forbidden": forbidden,
        "session_event_forbidden_seen": forbidden_seen,
        "session_event_tool_started_count": tool_start_count,
        "session_event_tool_result_completed_count": tool_result_count,
        "session_event_types": event_types,
        "session_event_error": None
        if ok
        else (
            "; ".join(count_errors)
            if count_errors
            else "required event order not observed or forbidden event seen"
        ),
    }


def assert_persistence_contract(
    *,
    db_path: Path,
    session_id: str | None,
    expected_outcome: str,
    expected_tool_part_count: int | None,
) -> dict[str, object]:
    if not session_id:
        return {
            "persistence_contract": "failed",
            "persistence_error": "no session id available",
        }
    roles = session_message_roles(db_path, session_id)
    parts = session_part_rows(db_path, session_id)
    errors: list[str] = []
    if "user" not in roles:
        errors.append("missing persisted user message")
    if expected_outcome == "completed" and "assistant" not in roles:
        errors.append("missing persisted assistant message for completed turn")
    if expected_tool_part_count is not None and len(parts) != expected_tool_part_count:
        errors.append(
            f"expected {expected_tool_part_count} persisted tool parts, got {len(parts)}"
        )
    if expected_outcome == "completed":
        incomplete = [
            part
            for part in parts
            if str(part.get("status") or "").lower() not in {"completed", "failed"}
        ]
        if incomplete:
            errors.append(f"completed turn has unsettled tool parts: {incomplete}")
    return {
        "persistence_contract": "passed" if not errors else "failed",
        "persistence_error": None if not errors else "; ".join(errors),
        "message_roles": roles,
        "session_tool_parts": parts,
    }


def assert_provider_repair_diagnostic(
    *,
    db_path: Path,
    session_id: str | None,
) -> dict[str, object]:
    if not session_id:
        return {
            "provider_repair_diagnostic_contract": "failed",
            "provider_repair_diagnostic_error": "no session id available",
        }
    diagnostics = session_runtime_diagnostics(db_path, session_id)
    repair_diagnostics = [
        diagnostic
        for diagnostic in diagnostics
        if diagnostic.get("schema") == "provider_tool_call_repair.v1"
    ]
    ok = any(
        int(diagnostic.get("malformed_tool_calls") or 0) > 0
        or int(diagnostic.get("argument_repairs") or 0) > 0
        or int(diagnostic.get("scavenged_tool_calls") or 0) > 0
        for diagnostic in repair_diagnostics
    )
    return {
        "provider_repair_diagnostic_contract": "passed" if ok else "failed",
        "provider_repair_diagnostic_error": None
        if ok
        else "provider_tool_call_repair.v1 diagnostic with repair counters not found",
        "provider_repair_diagnostics": repair_diagnostics,
    }


def terminal_contract_for_outcome(
    result: dict[str, object], expected_outcome: str
) -> dict[str, object]:
    if expected_outcome == "interrupted":
        ok = bool(result["saw_cancelled"]) and not bool(result["saw_tool_final_answer"])
        error = None if ok else "expected visible cancellation without final answer"
    elif expected_outcome == "provider-timeout":
        raw_error = bool(result["saw_error"]) or result.get("terminal_marker") in {
            "Error",
            "Failed to get response",
        }
        ok = raw_error and not bool(result["saw_tool_final_answer"])
        error = None if ok else "expected visible provider timeout/error without final answer"
    else:
        ok = (
            bool(result["saw_tool_final_answer"])
            and not bool(result["saw_cancelled"])
            and not bool(result["saw_error"])
        )
        error = None if ok else "expected visible final answer without cancellation/error"
    return {
        "terminal_contract": "passed" if ok else "failed",
        "terminal_contract_error": error,
    }


def run_smoke(args: argparse.Namespace, size: tuple[int, int]) -> dict[str, object]:
    cols, rows = size
    prompt = PROMPTS[args.prompt]
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    stem = f"tui-pty-{args.prompt}-{cols}x{rows}"
    ansi_path = out_dir / f"{stem}.ansi"
    text_path = out_dir / f"{stem}.txt"

    env = os.environ.copy()
    env.setdefault("TERM", "xterm-256color")
    env.setdefault("RUST_BACKTRACE", "0")
    command = f"stty rows {rows} cols {cols}; {args.binary} --tui"
    child = pexpect.spawn(
        "/bin/zsh",
        ["-fc", command],
        cwd=str(args.repo),
        env=env,
        encoding="utf-8",
        codec_errors="replace",
        timeout=1,
    )

    raw_parts: list[str] = []
    sent_prompt = args.prompt == "startup"
    sent_interrupt = False
    start = time.time()
    prompt_sent_at: float | None = None
    deadline = start + args.timeout
    settle_until: float | None = None
    terminal_marker: str | None = None

    while time.time() < deadline:
        try:
            chunk = child.read_nonblocking(size=8192, timeout=0.5)
            if chunk:
                raw_parts.append(chunk)
        except pexpect.TIMEOUT:
            pass
        except pexpect.EOF:
            break

        raw = "".join(raw_parts)
        elapsed = time.time() - start
        ready = (
            "Message Priority Agent" in raw
            or "Priority Agent" in raw
            or "? shortcuts" in raw
            or "session-" in raw
        )
        if not sent_prompt and (ready or elapsed > args.startup_grace):
            child.send(prompt)
            child.send("\r")
            raw_parts.append("\n__TUI_SMOKE_PROMPT_SENT__\n")
            sent_prompt = True
            prompt_sent_at = time.time()

        if (
            sent_prompt
            and not sent_interrupt
            and args.interrupt_after is not None
            and prompt_sent_at is not None
            and time.time() - prompt_sent_at >= args.interrupt_after
        ):
            if args.interrupt_key == "esc":
                child.send("\x1b")
            else:
                child.sendcontrol("c")
            raw_parts.append(f"\n__TUI_SMOKE_INTERRUPT_SENT:{args.interrupt_key}__\n")
            sent_interrupt = True

        if sent_prompt:
            raw = "".join(raw_parts)
            if args.prompt in TOOL_PROMPTS:
                terminal_markers = [
                    "Error",
                    "Failed to get response",
                    "Response ready",
                    "final answer",
                    "persisted",
                    "当前工作目录",
                    "Run interrupted",
                    "Cancelled",
                ]
            else:
                terminal_markers = [
                    "Reply",
                    "Error",
                    "Failed to get response",
                    "[Shell]",
                    "$ pwd",
                    "Ran pwd",
                    "Run interrupted",
                    "Cancelled",
                ]
            if args.prompt == "provider-ok":
                terminal_markers.append("OK")
            if args.prompt not in TOOL_PROMPTS:
                terminal_markers.append("Running  · waiting")
            matched_marker = next((marker for marker in terminal_markers if marker in raw), None)
            terminal = matched_marker is not None
            if terminal and settle_until is None:
                terminal_marker = matched_marker
                settle_until = time.time() + args.settle
            if settle_until is not None and time.time() >= settle_until:
                break

    if child.isalive():
        child.sendcontrol("c")
        end_deadline = time.time() + 5
        while time.time() < end_deadline and child.isalive():
            try:
                chunk = child.read_nonblocking(size=8192, timeout=0.5)
                if chunk:
                    raw_parts.append(chunk)
            except pexpect.TIMEOUT:
                pass
            except pexpect.EOF:
                break
    if child.isalive():
        child.terminate(force=True)

    raw = "".join(raw_parts)
    ansi_path.write_text(raw, encoding="utf-8", errors="replace")
    text_path.write_text(strip_ansi(raw), encoding="utf-8")

    result = {
        "size": f"{cols}x{rows}",
        "prompt": args.prompt,
        "ansi_path": str(ansi_path),
        "text_path": str(text_path),
        "sent_prompt": sent_prompt,
        "sent_interrupt": sent_interrupt,
        "terminal_marker": terminal_marker,
        "exitstatus": child.exitstatus,
        "signalstatus": child.signalstatus,
        "saw_reply": "Reply" in raw,
        "saw_ok": args.prompt == "provider-ok" and "OK" in raw,
        "saw_shell": "[Shell]" in raw or "$ pwd" in raw or "Ran pwd" in raw,
        "saw_tool_active": "Running  · waiting" in raw or "Running · queued" in raw,
        "saw_tool_result_observed": "result observed" in raw,
        "saw_tool_sent_back": "sent back to model" in raw
        or "sentback to model" in raw
        or "sent tool result to model" in raw,
        "saw_tool_final_answer": "final answer" in raw
        or "persisted" in raw
        or "Response ready" in raw
        or "当前工作目录" in raw,
        "saw_cancelled": terminal_marker in {"Run interrupted", "Cancelled"}
        or "[Cancelled:" in raw
        or "Run interrupted" in raw,
        "saw_slow_provider": "slow " in raw,
        "saw_error": "Error" in raw or "Failed to get response" in raw,
        "saw_raw_async_openai": "async_openai::error" in raw,
        "saw_deser": "failed deserialization of" in raw,
        "saw_display_provider": args.expect_provider_label in raw
        if args.expect_provider_label
        else None,
        "bytes": len(raw.encode("utf-8", errors="replace")),
    }
    expected_outcome = args.expect_outcome
    if expected_outcome == "auto":
        expected_outcome = "interrupted" if sent_interrupt else "completed"
    if args.assert_session_events and args.prompt in TOOL_PROMPTS:
        result.update(
            assert_tool_turn_events(
                db_path=Path(args.session_db).expanduser(),
                prompt=prompt,
                expected_outcome=expected_outcome,
                expected_tool_start_count=args.expect_tool_start_count,
                expected_tool_result_count=args.expect_tool_result_count,
            )
        )
    if args.assert_persistence:
        result.update(
            assert_persistence_contract(
                db_path=Path(args.session_db).expanduser(),
                session_id=result.get("session_id")
                if isinstance(result.get("session_id"), str)
                else None,
                expected_outcome=expected_outcome,
                expected_tool_part_count=args.expect_tool_part_count,
            )
        )
    if args.assert_provider_repair_diagnostic:
        result.update(
            assert_provider_repair_diagnostic(
                db_path=Path(args.session_db).expanduser(),
                session_id=result.get("session_id")
                if isinstance(result.get("session_id"), str)
                else None,
            )
        )
    if args.assert_terminal_contract:
        result.update(terminal_contract_for_outcome(result, expected_outcome))
    result["expected_outcome"] = expected_outcome
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--binary", default="target/debug/priority-agent")
    parser.add_argument("--build", action="store_true")
    parser.add_argument(
        "--size",
        type=parse_size,
        action="append",
        default=None,
        help="terminal size as COLSxROWS; can be repeated",
    )
    parser.add_argument(
        "--prompt",
        choices=sorted(PROMPTS),
        default="startup",
        help="smoke prompt to submit after startup",
    )
    parser.add_argument("--timeout", type=float, default=90)
    parser.add_argument("--startup-grace", type=float, default=18)
    parser.add_argument("--settle", type=float, default=4)
    parser.add_argument(
        "--interrupt-after",
        type=float,
        default=None,
        help="seconds after prompt submission before sending an interrupt key",
    )
    parser.add_argument("--interrupt-key", choices=["esc", "ctrl-c"], default="esc")
    parser.add_argument("--out-dir", default="target/tui-pty-smoke")
    parser.add_argument(
        "--assert-session-events",
        action="store_true",
        help="verify tool-turn session_events order in the session DB",
    )
    parser.add_argument(
        "--expect-tool-start-count",
        type=int,
        default=None,
        help="expected number of tool_started events for the selected session",
    )
    parser.add_argument(
        "--expect-tool-result-count",
        type=int,
        default=None,
        help="expected number of tool_result_completed events for the selected session",
    )
    parser.add_argument(
        "--assert-persistence",
        action="store_true",
        help="verify persisted messages and session_parts are consistent with the outcome",
    )
    parser.add_argument(
        "--expect-tool-part-count",
        type=int,
        default=None,
        help="expected number of persisted tool/shell session_parts",
    )
    parser.add_argument(
        "--assert-provider-repair-diagnostic",
        action="store_true",
        help="verify provider-boundary tool-call repair diagnostic was persisted",
    )
    parser.add_argument(
        "--expect-outcome",
        choices=["auto", "completed", "interrupted", "provider-timeout"],
        default="auto",
        help="event contract outcome to assert when --assert-session-events is set",
    )
    parser.add_argument(
        "--assert-terminal-contract",
        action="store_true",
        help="verify terminal-visible status matches --expect-outcome",
    )
    parser.add_argument(
        "--expect-provider-label",
        default="DeepSeek / deepseek-v4-flash",
        help="provider/model label expected to appear in the TUI; empty disables this check field",
    )
    parser.add_argument("--session-db", default=str(default_session_db_path()))
    args = parser.parse_args()

    args.repo = args.repo.resolve()
    ensure_binary(args.repo, args.binary, args.build)
    sizes = args.size or [(120, 35)]
    results = [run_smoke(args, size) for size in sizes]
    print(json.dumps(results, ensure_ascii=False, indent=2))
    if args.assert_session_events and any(
        result.get("session_event_contract") == "failed" for result in results
    ):
        return 1
    if args.assert_persistence and any(
        result.get("persistence_contract") == "failed" for result in results
    ):
        return 1
    if args.assert_provider_repair_diagnostic and any(
        result.get("provider_repair_diagnostic_contract") == "failed"
        for result in results
    ):
        return 1
    if args.assert_terminal_contract and any(
        result.get("terminal_contract") == "failed" for result in results
    ):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

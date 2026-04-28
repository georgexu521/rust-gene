# Live Execution Observation: memory-save-quality-gate

- Run id: `initial-minimax-context`
- Case: `memory-save-quality-gate`
- Attempted: `2026-04-29 00:00 +0800`
- Worktree: `target/live-evals/initial-minimax-context/memory-save-quality-gate/worktree`
- Outcome: aborted before code changes

## What Happened

The first real interactive run exposed a harness isolation problem rather than
a useful coding result.

The agent was launched from the prepared eval worktree, but its process still
used the developer's normal `HOME`, XDG config/data locations, session store,
and persistent memory. During the run, the model/tool flow began inspecting
absolute paths under the main repository:

```text
/Users/georgexu/Desktop/rust-agent/src/tools/memory_tool/mod.rs
```

instead of the eval worktree:

```text
target/live-evals/initial-minimax-context/memory-save-quality-gate/worktree
```

That makes the run invalid because global history or memory can leak into the
task and the agent can touch the wrong checkout.

## Model/API Issues Observed

MiniMax also produced several streaming/API failures during the run:

```text
failed deserialization ... missing field prompt_tokens
Failed to parse tool args: EOF while parsing a string
Stream interrupted: failed to deserialize api response
500 Internal Server Error
```

These should be tracked separately from task quality. They affect live
execution reliability but do not tell us whether the memory quality-gate task
was solved.

## Harness Fix

`scripts/run_live_eval.sh` now creates an isolated environment per case and
uses it for manual agent runs and API-plan runs:

```text
HOME
XDG_CONFIG_HOME
XDG_DATA_HOME
XDG_STATE_HOME
PRIORITY_AGENT_A2A_TRANSCRIPT_PATH
```

This prevents global sessions, persistent memory, and previous absolute-path
facts from contaminating future live evals.

## Next Check

Re-run this case in a fresh run id after the isolation patch. A valid run
should satisfy all of the following:

- agent starts with an empty per-case memory/session store
- all file reads and edits target the eval worktree
- no global `~/.priority-agent` or `~/.config/priority-agent` state is used
- final diff exists only inside the eval worktree
- required checks are collected from that same worktree

# Live Eval Summary: initial-minimax-context

- Run id: `initial-minimax-context`
- Mode: `api-plan`
- Provider under test: `MiniMax`
- Scope: 5 live task samples
- Generated: `2026-04-28`

## Result

All 5 MiniMax API planning calls completed successfully.

This run tests the planning-quality part of the live regression harness. The
runner now injects lightweight repository context before calling the local
Priority Agent API:

- current ref
- Rust project hint
- high-signal `src/**/*.rs` file list
- task-keyword `rg` hits

## Comparison With The First Smoke

Repository context materially improved file grounding.

Before context injection, MiniMax often guessed generic paths such as
`src/ui/`, `src/commands.rs`, or `src/skill_proposals/mod.rs`. With context,
plans now mention real files such as:

- `src/shell.rs`
- `src/tui/mod.rs`
- `src/tui/screens/main_screen.rs`
- `src/tools/memory_tool/`
- `src/memory/quality.rs`
- `src/memory/scoring.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/learning_planning.rs`
- `src/engine/retrieval_context.rs`
- `src/engine/skill_evolution.rs`
- `src/engine/evolution_controller.rs`

## Per-Task Notes

| Task | API | Grounding | Remaining Issues |
| --- | --- | --- | --- |
| `cli-scrollback-polish` | OK | Good | Correctly identified `src/shell.rs` and TUI files; still emitted `<think>`. |
| `memory-save-quality-gate` | OK | Good | Correctly identified memory tool/scoring/quality paths; still emitted `<think>`. |
| `persistent-memory-planning-context` | OK | Good | Correctly identified `conversation_loop`, `retrieval_context`, and `learning_planning`; still emitted `<think>`. |
| `resume-session-picker` | OK | Medium | Better session/memory paths, but still partly suggests new UI component before checking existing shell/session handlers; emitted `<think>`. |
| `skill-promotion-gate` | OK | Medium | Correctly found `skill_evolution` and `evolution_controller`; still vague on actual slash handler path; emitted `<think>`. |

## Findings

1. MiniMax is usable for live regression planning through the project API.
2. Repository context is necessary. Without it, planning file paths are too
   often plausible but wrong.
3. Format compliance is currently weak. All 5 plans still include `<think>`.
4. `api-plan` is useful as a cheap planning-quality gate, but it is not a
   substitute for live execution because no tools are actually run.

## Next Step

Use `initial-minimax-context` as the first planning baseline, then run one real
live execution task in a prepared worktree:

```bash
target/live-evals/initial-minimax-context/memory-save-quality-gate/worktree
```

For that task, collect:

- final diff
- required command output
- tool/trace observations
- human review score

Then repeat for the remaining four samples after the collection format is
stable.

## Live Execution Attempt 1

The first interactive attempt on `memory-save-quality-gate` was aborted before
code changes because the harness still reused global developer state. Although
the process started inside the eval worktree, the agent began inspecting
absolute paths under `/Users/georgexu/Desktop/rust-agent`, which invalidates the
run.

The runner has been updated so each case now has isolated `HOME`,
`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, and
`PRIORITY_AGENT_A2A_TRANSCRIPT_PATH` values. Future live execution attempts
should use the generated `RUNBOOK.md` command so global sessions and persistent
memory cannot leak into the task.

An isolated `api-plan` smoke was run after the patch. It completed successfully
and a search of the generated artifacts/env did not find leaked global
`~/.priority-agent`, `~/.config/priority-agent`, or main-checkout
`/Users/georgexu/Desktop/rust-agent/src/...` paths. The smoke artifacts were
discarded after verification.

Details:
`docs/benchmarks/live-initial-minimax-context/memory-save-quality-gate/live-execution-observation.md`

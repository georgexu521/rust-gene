# Priority Agent Project Notes

Only the `## Agent Runtime Guidance` section is intended for normal prompt
injection. Longer history and old product notes were archived in
`docs/archive/AGENTS_PROJECT_GUIDE_PRE_RUNTIME_DIET_2026-05-08.md`.

## Agent Runtime Guidance

## Current Project Facts

- Product: `priority-agent`, a Rust programming-agent terminal CLI.
- Default entry: `priority-agent` or `pa`; `--cli` is the normal interactive
  path. `--tui` is the compatibility full-screen terminal interface.
- Canonical status: `docs/PROJECT_STATUS.md`.
- Claude Code gap tracking:
  `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`.
- Runtime simplification plan:
  `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`.
- Product principle:
  `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`.

## Product Direction

Priority Agent is not trying to win by becoming a broad, generic clone of
Claude Code, Codex, opencode, or any other general-purpose coding agent.

The guiding principle is: narrow, deep, personal, and verifiable. Large vendors
will likely win generic entrypoints; this project should win by being the agent
that best understands gex's machine, projects, habits, validation loops, and
local coding workflow.

## Work Style

- Read the current code before changing behavior. Follow existing module
  boundaries and keep edits scoped to the requested phase.
- Preserve user or prior-agent work in the dirty tree. Do not revert unrelated
  changes.
- Prefer `rg` / `rg --files` for search and local targeted tests for feedback.
- Keep docs aligned only when a change affects startup, validation, or current
  project status.
- Do not force a heavyweight planning, priority, or workflow framework into
  simple tasks. Runtime checks, tool contracts, and tests should carry hard
  constraints; the model should keep normal problem-solving freedom.

## Main Entry Points

- Startup and mode routing: `src/main.rs`.
- Prompt assembly: `src/engine/prompt_context.rs`,
  `src/instructions/mod.rs`, `src/engine/mod.rs`.
- Main execution loop: `src/engine/conversation_loop/mod.rs`.
- Query and streaming paths: `src/engine/query_engine.rs`,
  `src/engine/streaming.rs`.
- Intent routing and workflow policy: `src/engine/intent_router.rs`,
  `src/engine/workflow/`.
- Tool registry and routing: `src/tools/mod.rs`.
- Memory and retrieval: `src/memory/manager.rs`,
  `src/engine/retrieval_context.rs`.
- Interactive CLI surface: `src/tui/`.

## Validation Commands

Use the narrowest gate that matches the change, then broaden when behavior or
shared contracts moved.

```bash
cargo check -q
cargo fmt --check
cargo test -q instructions
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
```

For workflow or live-eval scripts:

```bash
bash scripts/workflow-production-gates.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

## Current Cleanup Focus

The active line of work is reducing over-control in the LLM runtime:

- keep always-on prompts short and practical;
- move detailed behavioral rules into tool contracts and runtime checks;
- expose tools by route/role rather than by one broad default surface;
- keep memory, retrieval, and skills fenced as background context;
- keep user-facing final answers concise while traces retain debug evidence.

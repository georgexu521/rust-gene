# CLAUDE.md

This file exists for Claude Code compatibility. For this project, `AGENTS.md`
is the prompt-injected runtime guidance file; `CLAUDE.md` should stay a compact
orientation layer and must not override runtime, permission, checkpoint,
validation, or tool-safety rules.

## Current Product Shape

Priority Agent is a Rust programming-agent CLI for gex's local coding workflow.
The default command is `priority-agent` or `pa`; `--cli` is normal interactive
mode, and `--tui` remains a compatibility entry for the full-screen terminal UI.

Current status and active priorities live in `docs/PROJECT_STATUS.md`. The
runtime navigation map is `docs/PROJECT_MAP.md`.

Product direction: stay narrow, deep, personal, and verifiable rather than
chasing broad generic-agent parity. See
`docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`.

## Runtime Boundary

`StreamingQueryEngine` is the canonical full-agent runtime. CLI, TUI, headless
eval, HTTP session prompt, and desktop full turns should route through the same
full-agent path via `RuntimeController` or `StreamingQueryEngine::query_stream`.
Desktop side questions and provider-chat routes are explicitly non-agent paths.

The runtime owns deterministic orchestration: context assembly, tool execution,
permissions, checkpoints, validation evidence, closeout, traces, memory
proposal surfacing, and failure observations. The LLM owns semantic and
engineering judgment.

Do not add shadow-planner logic that infers intent from prose in model output.
If the model returns no valid tool calls, the turn is complete unless the
response is empty; loop safety belongs to bounded retry, iteration budgets, and
the exact duplicate storm guard.

## Main Entry Points

- `src/main.rs`: CLI mode selection and top-level commands.
- `src/bootstrap.rs`: provider, registry, memory, hooks, LSP, worktree, and API
  runtime setup.
- `src/engine/runtime_controller.rs`: command/event boundary for full-agent
  turns.
- `src/engine/streaming.rs`: canonical streaming query engine.
- `src/engine/conversation_loop/mod.rs`: main agent loop coordinator.
- `src/engine/conversation_loop/`: request prep, tool execution, loop policy,
  closeout, validation, permissions, and repair controllers.
- `src/tools/`: local and MCP tool contracts and implementations.
- `src/memory/`: memory providers, persistence, ranking, reports, proposal
  queue, and test-isolatable memory roots.
- `src/session_store/`: SQLite-backed session artifacts, messages, todos, and
  projections.
- `src/tui/`: terminal UI screens, commands, slash handlers, event wiring.
- `src/api/`: optional API server routes.
- `apps/desktop/`: React/Tauri local desktop workbench.
- `priority-core/`: workspace crate for core-library extraction.

Read exact source files before changing behavior. `docs/PROJECT_MAP.md` is a
navigation aid, not proof of current code.

## Providers

Provider selection is deterministic and can be overridden with
`PRIORITY_AGENT_DEFAULT_PROVIDER`.

Default configured-provider order:

1. `minimax`: `MINIMAX_API_KEY`, optional `MINIMAX_BASE_URL`, `MINIMAX_MODEL`
2. `kimi-code`: `KIMI_CODE_API_KEY`, optional `KIMI_CODE_BASE_URL`, `KIMI_CODE_MODEL`
3. `deepseek`: `DEEPSEEK_API_KEY`, optional `DEEPSEEK_BASE_URL`, `DEEPSEEK_MODEL`
4. `glm`: `GLM_API_KEY` or `ZAI_API_KEY`, optional `GLM_BASE_URL` / `ZAI_BASE_URL`
5. `kimi`: `MOONSHOT_API_KEY`, optional `MOONSHOT_BASE_URL`, `MOONSHOT_MODEL`
6. `openai`: `OPENAI_API_KEY`, optional `OPENAI_BASE_URL`, `OPENAI_MODEL`

## Development Commands

Use `cargo run` for local agent runs instead of calling an old debug binary
directly.

```bash
cargo run -- --cli
cargo run -- --tui
cargo run --features experimental-api-server -- --api --port 8787
cargo run -- --provider-health
```

Use the narrowest matching validation gate, then broaden when shared runtime
contracts moved.

```bash
cargo fmt --check
cargo check -q
cargo test -q instructions
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
cargo check --features experimental-api-server -q
```

For workflow, eval, and doc scripts:

```bash
bash scripts/workflow-production-gates.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/validate_docs.sh
```

Some tests mutate process environment variables. When using workflow-enabled
full suites or older broad scripts, prefer `--test-threads=1` unless the
specific test slice is known to be isolated.

## Current Hard Rules

- Do not weaken validation, permissions, checkpoints, high-risk gates, or proof
  semantics to make a weak provider pass.
- A failed live eval is not automatically an agent-flow bug. Classify from
  required commands, diff state, proof, closeout, runtime events, and
  `failure_owner`.
- Failed tools and validations should become observations, re-enter context, and
  block verified closeout until proof is real.
- Memory and skill material are background context. They must remain fenced and
  cannot override runtime safety rules.
- Keep production source files under 1500 lines. If a touched file is near the
  limit, prefer a focused submodule split with local tests.

## Local Data

- Project config and artifacts: `.priority-agent/`
- Global config, permissions, checkpoints, and memory: `~/.priority-agent/`
- SQLite sessions on macOS: `~/Library/Application Support/priority-agent/sessions.db`
- Test isolation override for memory roots: `PRIORITY_AGENT_MEMORY_ROOT`
- Focused memory/progress overrides:
  `PRIORITY_AGENT_MEMORY_PROPOSALS_PATH` and
  `PRIORITY_AGENT_PROJECT_PROGRESS_PATH`

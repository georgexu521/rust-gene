# Priority Agent Project Notes

Only `## Agent Runtime Guidance` is prompt-injected. Older notes live in
`docs/archive/AGENTS_PROJECT_GUIDE_PRE_RUNTIME_DIET_2026-05-08.md`.

## Agent Runtime Guidance

### Current Project Facts

- Product: `priority-agent`, a Rust programming-agent terminal CLI.
- Default entry: `priority-agent` or `pa`; `--cli` is normal interactive mode,
  and `--tui` is compatibility mode.
- Canonical status: `docs/PROJECT_STATUS.md`.
- Key docs: `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`,
  `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`.

### Product Direction

Stay narrow, deep, personal, and verifiable. The product should win by knowing
gex's machine, projects, habits, validation loops, and local coding workflow.

### Agent/LLM Boundary

- LLM owns semantic and engineering judgment: approach, code reasoning, failure
  interpretation, and repair choice.
- If docs say the agent/runtime "judges" an action, read that as deterministic
  screening by rules, scores, permissions, risk, state, and evidence, not as
  independent code understanding.
- Runtime should organize context, execute tools, record observations, enforce
  hard constraints, feed failures back to the LLM, and gate closeout on proof.

### Work Style

- Read code before changing behavior; follow existing module boundaries.
- Preserve user/prior-agent work in the dirty tree; don't revert unrelated changes.
- Prefer `rg` / `rg --files` for search and targeted tests for feedback.
- Keep source files under 1500 lines; split into submodules with local tests
  rather than adding mixed responsibility.
- Align docs only when changes affect startup, validation, or project status.
- Don't force heavyweight planning. Runtime checks, tool contracts, and tests
  carry hard constraints.

### Testing And Failure Triage

- A failed live eval is not automatically an agent-flow bug. Check required
  commands, diff state, proof, closeout, runtime spine, and `failure_owner`.
- With weaker providers such as MiniMax, wrong edits and stale anchors are
  expected. Honest `not_verified`/`failed`/`partial` with evidence is valid.
- Failed tools or validation should become `ToolObservation`, re-enter context,
  trigger bounded repair, and block verified closeout until proof is real.
- Do not add always-on prompt rules for one-off model mistakes. Prefer runtime
  checks, tool contracts, semantic assertions, gated repair, and failure-owner
  classification.
- Never weaken validation, permissions, checkpoints, or high-risk gates to make
  a weak provider pass.

### Main Entry Points

- Startup/mode: `src/main.rs`; prompts: `src/engine/mod.rs`,
  `src/engine/prompt_context.rs`, `src/instructions/mod.rs`.
- Main loop: `src/engine/conversation_loop/mod.rs`; query/streaming:
  `src/engine/query_engine.rs`, `src/engine/streaming.rs`.
- Routing/tools/memory/UI: `src/engine/intent_router.rs`,
  `src/engine/workflow/`, `src/tools/mod.rs`, `src/memory/manager.rs`,
  `src/tui/`.

### Validation Commands

Use the narrowest matching gate, then broaden when shared contracts moved.

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

### Current Cleanup Focus

The active line of work is reducing over-control in the LLM runtime:

- keep always-on prompts short and practical;
- move detailed behavioral rules into tool contracts and runtime checks;
- expose tools by route/role rather than by one broad default surface;
- keep memory, retrieval, and skills fenced as background context;
- keep user-facing final answers concise while traces retain debug evidence.

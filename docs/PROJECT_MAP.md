# Project Map

Compact navigation map for `priority-agent`. This file is an agent entry point,
not proof of current code. Read exact files before editing.

<!-- agent-context:start -->

## Runtime Navigation Contract

- Start here for orientation before broad repository scans.
- Use `project_list` action `map` or the injected project-map zone to pick likely files.
- Use `symbol_query` for functions, structs, traits, enums, and impls before broad `grep`.
- Use `file_read` on exact files/ranges before edits; this map is not source truth.
- When changing module boundaries, startup behavior, runtime context, tools, or validation flows, update this file in the same change.

## Product Shape

- Product: Rust terminal programming agent CLI named `priority-agent` / `pa`.
- Default mode: `--cli`; `--tui` remains compatibility mode.
- Runtime principle: LLM owns judgment; deterministic runtime organizes context, tools, evidence, validation, permissions, and closeout gates.
- Canonical status doc: `docs/PROJECT_STATUS.md`.

## Top-Level Layout

- `src/main.rs`: CLI entry and mode selection.
- `src/bootstrap.rs`: shared startup wiring for registry, providers, memory, hooks, and runtime managers.
- `src/engine/`: conversation loop, prompt/context assembly, routing, workflow, tracing, verification, retrieval, and runtime policy.
- `src/tools/`: tool contracts and implementations exposed to the model.
- `src/services/api/`: provider adapters, request/response normalization, prompt cache accounting, weak-model tool-call repair.
- `src/memory/`: memory providers, manager, persistence, retrieval, extraction, ranking, reports.
- `src/tui/`: terminal UI screens, commands, slash handlers, event wiring.
- `src/api/`: optional API server routes and tool allowlist behavior.
- `apps/desktop/`: React/Tauri frontend app for the local workbench experience.
- `scripts/`: local validation, live-eval, benchmark, and maintenance scripts.
- `docs/`: current status, design notes, archived plans, eval reports, and project map.
- `tests/`: integration and behavior tests outside module-local unit tests.

## Main Runtime Entry Points

- `src/engine/mod.rs`: engine module registry plus `default_system_prompt()`.
- `src/engine/query_engine.rs`: high-level query engine orchestration.
- `src/engine/streaming.rs`: streaming request/response path.
- `src/engine/conversation_loop/mod.rs`: main agent loop coordinator.
- `src/engine/conversation_loop/main_loop_profile.rs`: quiet-direct vs standard main-loop profile selection.
- `src/engine/conversation_loop/request_preparation_controller.rs`: request message preparation, dynamic context zones, model-led weighting hints, memory prefetch, cache-stability snapshot.
- `src/engine/conversation_loop/tool_execution_controller.rs`: tool execution, observations, checkpoints, action review, permission integration.
- `src/engine/conversation_loop/closeout_controller.rs`: final closeout, execution reports, memory proposal preparation.

## Context And Cache

- `src/engine/context_assembly.rs`: typed context zones, token reports, stable-prefix/dynamic-tail accounting.
- `src/engine/prompt_context.rs`: prompt assembly reports and stable fingerprints.
- `src/engine/cache_stability.rs`: provider tool schema canonicalization and prompt-cache accounting helpers.
- `src/engine/context_ledger.rs`: recent file/tool/validation evidence converted back into compact turn context.
- `src/engine/retrieval_context.rs`: retrieval items and prompt formatting.
- `src/engine/project_map.rs`: bounded `docs/PROJECT_MAP.md` runtime snippet, env budget controls, watched-path freshness detection, and machine-readable symbol/file index building.

## Routing, Workflow, And Safety

- `src/engine/intent_router.rs`: intent, retrieval policy, workflow, confidence, and risk routing.
- `src/engine/turn_ingress.rs`: desktop ingress classifier for explicit side questions and normal main-loop tasks.
- `src/engine/task_context.rs`: task state and task context bundle.
- `src/engine/task_contract.rs`: executor contract, context pack, validation requirements.
- `src/engine/tool_orchestration.rs`: route/tool planning and tool exposure intent.
- `src/engine/conversation_loop/tool_exposure_plan.rs`: route-scoped and stage-scoped tool exposure; programming `Understand` can expose `file_write` for new files while `file_edit`/`file_patch` stay edit-stage tools.
- `src/engine/action_decision.rs`: deterministic tool action scoring.
- `src/engine/candidate_action.rs`: model-proposed candidate action parsing, shadow/gated ranking, and model factor calibration.
- `src/engine/action_review.rs`: action review before execution.
- `src/engine/destructive_scope.rs`: destructive scope checks.
- `src/engine/verification_proof.rs`: verification proof model.
- `src/engine/auto_verify.rs`: automatic validation command selection.

## Tool Navigation

- `src/tools/mod.rs`: tool trait, registry, default tool registration.
- `src/tools/file_tool/`: file read/write/edit/patch and path resolution.
- `src/tools/grep_tool/`: text search; prefer after map/symbol narrowing.
- `src/tools/glob_tool/`: file globbing.
- `src/tools/project_tool/`: cached project file index, `project_list` summary/search/dir/map/index.
- `src/tools/symbol_tool/`: `symbol_query` using tree-sitter symbol indexing.
- `src/engine/symbol_index.rs`: project-level AST symbol index for Rust/TS/JS/Python, including Rust type identifiers.
- `src/tools/bash_tool/`: shell execution and background task handling.
- `src/tools/git_read_tool.rs` and `src/tools/diff_tool/`: git status/diff/read-only inspection.

## Frontend Workbench

- `apps/desktop/src/app/App.tsx`: main React shell, session/workspace state, run submission, drawers, workbench snapshot refresh.
- `apps/desktop/src/app/components/WorkbenchPanel.tsx`: local web workbench surface for project map, symbol index, runtime context, and cache surface.
- `apps/desktop/src/app/components/WorkbenchDrawer.tsx`: right-side workbench drawer that keeps diagnostics and project intelligence out of the main chat flow.
- `apps/desktop/src/runtime/desktopApi.ts`: Tauri invoke wrappers plus web-preview fixtures.
- `apps/desktop/src-tauri/src/lib.rs`: desktop commands, selected project/session state, runtime bridge, workbench snapshot command.
- `apps/desktop/tests/desktop-ui-smoke.spec.ts`: Playwright layout and workflow smoke coverage.

## Provider And Weak-Model Boundaries

- `src/services/api/mod.rs`: shared provider request/response types.
- `src/services/api/provider.rs`: provider registry and configuration.
- `src/services/api/openai_compat.rs`: OpenAI-compatible conversion path.
- `src/services/api/minimax.rs`: MiniMax provider conversion path.
- `src/services/api/kimi.rs`: Kimi provider conversion path.
- `src/services/api/tool_call_repair.rs`: weak-model tool-call repair and schema flatten/unflatten support.
- `src/services/api/prompt_cache.rs`: prompt-cache usage extraction.

## Memory And Learning

- `src/memory/manager.rs`: memory manager facade and core persistence/retrieval coordination.
- `src/memory/provider.rs`: memory provider traits and local provider behavior.
- `src/memory/persistence.rs`: local persistence primitives.
- `src/memory/ranking.rs`: memory ranking and scoring.
- `src/memory/reports.rs`: memory reports.
- `src/engine/improvement.rs`: self-evolution guidance selection.
- `src/engine/evolution_controller.rs`: evolution controller and proposals.
- `src/engine/experience_ledger.rs`: runtime experience records.

## Common Validation

- `cargo fmt --check`: formatting.
- `cargo check -q`: compile gate.
- `cargo test -q project_map`: project-map slice tests.
- `cargo test -q project_tool`: project-list/map/index tool tests.
- `cargo test -q request_preparation_controller`: request context-zone behavior.
- `cargo test -q prompt_context`: prompt/context reporting.
- `corepack pnpm --dir apps/desktop build`: desktop frontend typecheck and production build.
- `corepack pnpm --dir apps/desktop test:ui-smoke`: desktop web-preview Playwright smoke tests.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q`: Tauri shell compile gate.
- `cargo test -q`: broad Rust tests when shared runtime contracts moved.
- `cargo clippy --all-features -- -D warnings`: broad lint gate.

## Update Triggers

- Update this file when adding, moving, or removing major modules, tools, provider paths, context-zone behavior, validation gates, startup wiring, or canonical docs.
- Keep entries one-line and navigational. Put rationale in design docs, not here.
- Do not paste generated full-file content into this map. Large or volatile detail belongs in code, tests, traces, or on-demand symbol/file reads.

<!-- agent-context:end -->

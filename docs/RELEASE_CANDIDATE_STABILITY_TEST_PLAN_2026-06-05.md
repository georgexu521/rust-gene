# Release Candidate Stability Test Plan
Status: Active

Date: 2026-06-05

## 1. Purpose

Priority Agent has finished several large hardening lines: runtime
simplification, memory controls, provider cost tracking, opencode-style
programming-chain contracts, route-scoped tools, file-size stewardship, TUI and
desktop entrypoint alignment, and daily deterministic gates.

The next phase should test whether the product can be used reliably, not add
more core behavior. This document defines a release-candidate stability pass:
from a clean local checkout through deterministic gates, real provider runs,
TUI/desktop paths, package rehearsal, cost review, and recovery behavior.

The goal is to answer:

- Can gex start from a clean project folder and run the product without hidden
  local state?
- Can the agent complete normal coding tasks with correct edits, evidence, and
  validation?
- Can failures be diagnosed from trace, usage, checkpoints, and session state?
- Are provider slow-tail, token cost, and cache behavior visible enough to
  explain real sessions?
- Is the project ready for daily dogfood and a first release candidate?

## 2. Testing Principles

- Test stability before features.
- Prefer repeatable commands and saved artifacts over impressions.
- Separate deterministic product bugs from provider/model quality failures.
- Never weaken validation, permissions, checkpointing, or proof requirements to
  make a run pass.
- Use the same model/prompt/settings when comparing provider cost or speed.
- Keep weak-model mistakes classified as model/provider outcomes unless the
  runtime flow itself lost evidence, applied an unsafe edit, skipped validation,
  or reported a false verified closeout.

## 3. Test Lanes

### Lane A: Clean Checkout And Cold Build

Purpose: prove the project can rebuild after deleting all local generated
artifacts.

Precondition:

- `target/`, `apps/desktop/src-tauri/target/`, `apps/desktop/node_modules/`,
  `apps/desktop/dist/`, `apps/desktop/test-results/`, and
  `apps/desktop/test-artifacts/` may be absent.
- Git worktree is clean.

Commands:

```bash
git status --short --branch
cargo check -q
cargo test -q instructions -- --test-threads=1
```

Desktop dependency restore, when desktop testing is included:

```bash
corepack pnpm --dir apps/desktop install
corepack pnpm --dir apps/desktop build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

Pass criteria:

- Clean Rust build succeeds from no `target/`.
- Desktop dependencies install without manual lockfile edits.
- Desktop build and Tauri compile succeed after `node_modules` and Tauri
  `target` are regenerated.

Failure classification:

- Build error in source: product bug.
- Missing system dependency: setup-doc gap.
- Network/package registry failure: environment issue, retry once and record.

### Lane B: Deterministic Daily Baseline

Purpose: prove the deterministic runtime spine is healthy before spending
provider tokens.

Command:

```bash
bash scripts/daily-baseline.sh
```

Required gates:

- `cargo check -q`
- `cargo check --features experimental-api-server -q`
- `cargo fmt --check`
- `cargo test --lib -q instructions`
- `cargo test --lib -q cache_stability`
- `cargo test --lib -q runtime_controller`
- `cargo test --lib -q route_scoped_tools`
- `cargo test --lib -q closeout`
- `cargo test --lib -q permissions`
- `cargo test --lib -q checkpoint`
- `cargo test --lib -q file_tool`
- `cargo test --lib -q desktop_runtime`
- `cargo test --lib -q usage_ledger`
- `cargo test --lib -q cost_tracker`
- `cargo test --lib -q edit_match`
- `cargo test --lib -q`
- `bash scripts/file-size-report.sh --threshold 1200 --top 25`
- script syntax checks for live eval, parser, and daily baseline.

Pass criteria:

- All gates pass.
- File-size report has no new production source file above 1500 lines unless
  already documented as an exception.
- Existing test-helper warnings are acceptable only if `clippy -D warnings`
  remains clean.

### Lane C: CLI Coding Smoke

Purpose: test the default user path with real tools, edits, validation, and
closeout.

Scenario C1: read, edit, validate.

Task:

- In a temporary fixture repository, ask the agent to inspect a small Rust file,
  make a narrow edit, and run a required validation command.

Expected behavior:

- Reads target before editing.
- Uses `file_edit` or `file_patch` with exact or deterministic-safe recovery.
- Produces `mutation_result`.
- Creates checkpoint/file-change evidence.
- Runs validation.
- Final answer reports verified status only if validation actually passed.

Scenario C2: stale anchor recovery.

Task:

- Give the agent an edit where whitespace/indentation changed from the prompt.

Expected behavior:

- Unique stale-safe multi-line candidate can auto-apply.
- Ambiguous candidates are refused with candidate diagnostics.
- Single-line whitespace-normalized matches stay hints, not silent fuzzy edits.

Scenario C3: todo and active-task continuity.

Task:

- Ask for a 3-step coding change.

Expected behavior:

- `todo_write` persists todos through session store.
- At most one item is `in_progress`.
- `/active-task` or session status can show current progress without parsing
  free-form tool output.
- Missing `priority` remains valid and defaults safely.

Suggested commands:

```bash
cargo run -- --cli
```

Artifacts to save:

- Command transcript or session id.
- `git diff` of fixture repo.
- Validation command output.
- `/trace last` or equivalent trace summary.
- `/cost` summary for the session.

### Lane D: TUI Real Path

Purpose: verify the compatibility TUI path still runs through the full agent
runtime and renders key product evidence.

Command:

```bash
cargo run -- --tui
```

Test cases:

- Submit a normal coding task.
- Confirm tool cards render file mutation summaries from `mutation_result`.
- Confirm usage/cost events appear in status surfaces.
- Confirm todo progress can survive a session reload when the same session is
  reopened.
- Confirm long tool output remains inspectable without flooding the main chat.

Pass criteria:

- TUI does not bypass `StreamingQueryEngine`.
- Tool cards do not require parsing ad hoc file tool strings when
  `mutation_result` exists.
- A failed validation blocks verified closeout.
- The UI remains responsive during a long-running foreground or background
  shell command.

### Lane E: Desktop Real Path

Purpose: verify the desktop bridge is thin and shares runtime behavior with CLI
and TUI.

Commands:

```bash
corepack pnpm --dir apps/desktop install
corepack pnpm --dir apps/desktop build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

If running dev mode:

```bash
corepack pnpm --dir apps/desktop dev
```

Test cases:

- Full-agent desktop turn completes a small coding task.
- Lightweight side question remains explicitly non-agent and does not claim tool
  use.
- Runtime diagnostic, usage, permission, closeout, and tool completion events
  show in desktop status surfaces.
- Session DB can be inspected through a copied snapshot if the live DB is
  locked.

Pass criteria:

- Desktop full turn reaches `DesktopRuntime::run_full_turn()`.
- Desktop lightweight path remains no-tools, low-token, and clearly separate.
- Desktop does not invent a second agent lifecycle.

### Lane F: Provider Cost And Slow-Tail Baseline

Purpose: replace token/cost impressions with a real usage ledger comparison.

Baseline prompt:

Use one stable coding prompt across all provider comparisons. Example:

```text
In this temporary Rust crate, inspect the code, fix the failing unit test with
the smallest correct change, run the relevant test, and summarize the evidence.
```

Control variables:

- Same model id.
- Same base URL/provider when comparing runtime policy changes.
- Same output cap.
- Same reasoning/thinking effort when supported.
- Same initial tool surface.
- Same fixture repository and clean git state.

Record per run:

- session id
- model
- prompt tokens
- completion tokens
- cached tokens/cache hit
- cache miss tokens
- tool schema tokens
- request phase
- effective output cap
- tool round count
- compaction decision
- total wall time
- slowest provider call
- validation result
- final closeout status
- failure owner if failed

Commands and surfaces:

```bash
cargo run -- --cli
# after run:
# /cost
# /cache miss-report
# /trace last
```

Pass criteria:

- Usage ledger records all provider calls that report usage.
- Main streaming requests record effective output cap and request phase.
- `/cost` can explain whether a costly run came from prompt miss, schema
  tokens, completion tokens, repeated tool rounds, or long context.
- Slow-tail provider calls are visible enough to debug without reading raw logs.

### Lane G: Recovery And Safety

Purpose: prove important failures are recoverable and do not turn into false
success.

Test cases:

- Provider stream interruption triggers non-streaming fallback once.
- Context overflow or provider context error triggers reactive compaction retry.
- Tool failure returns `ToolObservation` and re-enters the model context.
- Failed validation produces focused repair or honest `not_verified`.
- Permission denial blocks the action and is visible in trace/final answer.
- Checkpoint restore/round rewind can explain affected paths without needing
  internal checkpoint ids.

Pass criteria:

- No false verified closeout.
- No raw destructive action bypasses permission/checkpoint gates.
- Failed tools are visible to the model and trace.
- Recovery attempts are bounded.

### Lane H: Package Rehearsal

Purpose: verify a user can install or launch a packaged artifact.

CLI package checks:

```bash
cargo build --release
./target/release/priority-agent --help
./target/release/priority-agent --provider-health
```

Desktop package checks:

```bash
corepack pnpm --dir apps/desktop install
corepack pnpm --dir apps/desktop build
# run the repository's packaging script when ready:
bash scripts/package-macos-app.sh
```

Pass criteria:

- Release binary starts.
- Help/version/provider-health work.
- Packaged desktop app launches and can select a project.
- Setup docs mention that `target/`, Tauri `target/`, and `node_modules` are
  local rebuildable artifacts.

## 4. Minimum RC Gate

Run this before calling a branch release-candidate ready:

```bash
git status --short --branch
bash scripts/daily-baseline.sh
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
corepack pnpm --dir apps/desktop install
corepack pnpm --dir apps/desktop build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

Then complete at least these manual runs:

- one CLI coding task with edit + validation;
- one TUI coding task with visible mutation card;
- one desktop full-agent task;
- one desktop lightweight side question;
- one provider-cost run with `/cost`, `/cache miss-report`, and `/trace last`;
- one recovery run: failed validation or stale edit anchor.

## 5. Evidence Folder

Store release-candidate evidence under a timestamped folder:

```text
docs/benchmarks/rc-stability-YYYYMMDD-HHMMSS/
```

Recommended files:

```text
README.md
environment.txt
git-status.txt
daily-baseline.log
clippy.log
desktop-build.log
cli-coding-task.md
tui-coding-task.md
desktop-full-turn.md
provider-cost-summary.md
recovery-case.md
known-issues.md
```

Do not store raw provider API payloads, secrets, full system prompts, or large
generated build artifacts in docs.

## 6. Pass/Fail Rules

Release candidate is green when:

- Minimum RC gate passes.
- CLI, TUI, and desktop full-agent paths each complete one real task.
- At least one real provider run has a usage ledger and cost explanation.
- No test finds false verified closeout.
- No test finds unsafe edit application.
- No test finds permission/checkpoint bypass.
- No normal production source file newly exceeds 1500 lines.
- Known issues are documented and classified as non-blocking.

Release candidate is blocked when:

- Any deterministic daily-baseline gate fails.
- `cargo clippy --all-features -- -D warnings` fails.
- A file mutation can apply an unsafe or ambiguous edit silently.
- A failed validation can still produce verified closeout.
- TUI or desktop uses a divergent non-agent path for normal full-agent tasks.
- Usage/cost cannot explain provider token consumption for a real run.
- Session DB corruption or migration failure blocks normal startup.

## 7. Suggested One-Week Schedule

Day 1: Cold build and deterministic baseline.

- Rebuild after deleting generated artifacts.
- Run daily baseline and clippy.
- Fix only deterministic failures.

Day 2: CLI coding smoke.

- Run three fixture coding tasks.
- Record trace, diff, validation, and cost.
- Fix unsafe edit or false closeout issues immediately.

Day 3: TUI path.

- Run one normal coding task and one long command task.
- Check tool cards, mutation summaries, todo progress, and closeout status.

Day 4: Desktop path.

- Build desktop frontend and Tauri shell.
- Run full-agent and lightweight turns.
- Check event parity and session persistence.

Day 5: Provider cost and slow-tail.

- Run controlled DeepSeek or OpenAI-compatible fixture.
- Compare capped vs uncapped only if explicitly configured.
- Summarize prompt/cache/completion/schema/tool-round cost.

Day 6: Recovery and package rehearsal.

- Force stale anchor, failed validation, stream fallback, and permission denial
  scenarios.
- Build release CLI and package desktop if scripts are ready.

Day 7: RC decision.

- Write `known-issues.md`.
- Decide: release candidate green, blocked, or needs one more focused fix
  slice.

## 8. Non-Goals

- Do not add new agent features during the stability pass.
- Do not broaden the tool surface just to make a single task pass.
- Do not weaken validation or permission gates.
- Do not rewrite memory behavior unless a stability test proves a real defect.
- Do not force shell structural parser work into this RC unless current
  classifier/checkpoint behavior fails a documented safety case.
- Do not keep generated build artifacts in git or docs.

## 9. Immediate Next Action

Start with the cold-build deterministic gate:

```bash
git status --short --branch
cargo check -q
bash scripts/daily-baseline.sh
cargo clippy --all-features -- -D warnings
```

If this passes, move to one CLI coding smoke task and one provider-cost run.
If it fails, fix only the failing deterministic contract before running any
live provider tests.

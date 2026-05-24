# Desktop App Dogfood Test Plan

Date: 2026-05-23

Goal: start testing through the macOS desktop app, not only through CLI scripts,
so every run can find both agent-runtime defects and desktop UX defects.

This plan consolidates:

- `docs/EXTERNAL_BASELINE_TEST_PLAN_2026-05-22.md`
- `docs/MAC_DESKTOP_APP_PLAN_2026-05-22.md`
- `docs/DESKTOP_AGENT_TIMELINE_PLAN_2026-05-22.md`
- `docs/CLAUDE_CODE_ALGORITHM_GAP_PLAN_2026-05-23.md`
- `scripts/release-dogfood-gate.sh`

## Current Status

The code-level reliability work from
`docs/CLAUDE_CODE_ALGORITHM_GAP_PLAN_2026-05-23.md` is implemented and covered
by targeted tests. The missing validation is a full live dogfood pass.

The desktop app is the preferred test surface for the next phase. CLI/script
gates still matter as deterministic checks, but the product question is whether
the desktop app can support daily coding work with clear runtime evidence,
permission handling, session recovery, trace debugging, and readable UI.

## Test Principle

Each dogfood task should answer two questions:

1. Did the coding agent do the task correctly?
2. Did the desktop app make the run understandable, controllable, and
   recoverable?

Do not mark a case as passed just because the final assistant message claims
success. A pass needs observable evidence: diff, command output, validation,
permission decision, trace/timeline state, or preserved context.

## Test Matrix

### A. Desktop Release Dogfood Cases

These are the six current release dogfood cases from
`scripts/release-dogfood-gate.sh`. Run them first through the desktop app.

| Case | Purpose | Desktop-specific checks |
| --- | --- | --- |
| `core-simple-stale-edit` | Read-before-edit and focused single-file repair. | Timeline shows read/edit order; file card shows changed file; final answer cites validation. |
| `core-rust-multi-file-refactor` | Multi-file Rust repair with tests. | Multiple file changes are grouped clearly; validation command card is readable; trace can explain failures. |
| `desktop-ui-smoke-polish` | Desktop build/UI smoke task. | App can run its own desktop tests; long output stays collapsed; screenshots/results are discoverable. |
| `code-change-verification-repair-loop` | Failed verification must trigger repair before closeout. | Failed validation appears as failure, not success; repaired validation is visually distinct; final answer follows the process timeline. |
| `core-permission-rejection-recovery` | User rejects risky action; agent recovers safely. | Permission card is actionable in transcript; denial state/recovery is clear; no false claim that denied action ran. |
| `core-long-output-artifact` | Long command output and artifact handling. | Long output preview is collapsed; artifact path/evidence is visible; app remains responsive. |

Reference CLI validation remains:

```bash
scripts/release-dogfood-gate.sh quick
scripts/release-dogfood-gate.sh agent-run --run-tests --timeout 2400
scripts/release-dogfood-gate.sh summary --run-id <run-id>
```

For this phase, the primary run should be manual or semi-manual through the
desktop app. Use the CLI `agent-run` as a comparison or after desktop issues
are captured.

### B. External Baseline Scenarios From 2026-05-22

These six cases were originally created for Claude Code/Codex comparison. They
are also high-value desktop dogfood scenarios because they stress features that
users notice.

| Scenario | Desktop app objective |
| --- | --- |
| `file_edit_rewind` | Show edit, validation, and undo/rewind evidence clearly. |
| `bash_background_task` | Show background task handle/output/stop evidence without blocking the UI. |
| `permission_denial_retry` | Let the user deny a risky request and see safe recovery in the same transcript. |
| `compaction_boundary` | Preserve task facts after context pressure; show compaction/context evidence in trace. |
| `subagent_worktree_worker` | If supported, make delegation/worktree state reviewable; otherwise record blocked honestly. |
| `mcp_auth_repair` | Surface auth failure, repair guidance, approval, and retry evidence. |

These can still be used later for external Claude Code/Codex baseline capture,
but for the next local phase they should be run in Priority Agent Desktop first.

### C. Desktop UX Regression Checklist

Run this checklist during every dogfood case:

- Startup restores the expected project/session, or clearly says it started a
  new conversation.
- Provider/model/permission mode are visible before sending.
- Composer attached context chips are visible after send in the run header.
- Timeline cards distinguish process, failures, permissions, file edits,
  validation, and final answer.
- Trace drawer opens from timeline debug links and points to the relevant event.
- Settings page still shows project, session, provider status, diagnostics,
  permission defaults, and recent projects.
- Search/session switching does not lose the current run state.
- Long output does not freeze the app or flood the transcript.
- Permission approval/rejection works from the transcript, not only from a
  footer card.
- After a failed run, the UI makes the next corrective action clear.

## Evidence Template

Record one row per desktop dogfood run. Use `target/desktop-dogfood/` for local
notes, screenshots, and raw artifacts. Keep screenshots and transcripts out of
source control unless they are intentionally sanitized.

```text
Run id:
Date:
Desktop build/source commit:
Case:
Project/worktree:
Provider/model:
Permission mode:
Outcome: pass | fail | blocked | not_run

Agent correctness:
- Changed files:
- Validation commands:
- Permission events:
- Recovery/repair events:
- Final answer evidence:

Desktop UX:
- Timeline clarity:
- Trace/debug clarity:
- Composer/context behavior:
- Settings/diagnostics behavior:
- Layout/screenshot notes:

Defects found:
- [runtime] ...
- [desktop-ui] ...
- [desktop-runtime-bridge] ...
- [test-harness] ...

Next fix:
```

## Suggested Execution Workflow

1. Start from a clean-ish repo state and current desktop build.

```bash
git status --short
scripts/release-dogfood-gate.sh quick
corepack pnpm --dir apps/desktop build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
```

2. Launch the desktop app from the repo root.

```bash
PRIORITY_AGENT_DESKTOP_PROJECT_DIR=/Users/georgexu/Desktop/rust-agent \
  corepack pnpm --dir apps/desktop tauri dev
```

3. Prepare isolated fixtures where possible.

```bash
scripts/release-dogfood-gate.sh prepare --label desktop-dogfood
```

4. Run one case at a time through the desktop composer. Prefer the existing
   release dogfood prompts from `evalsets/live_tasks/*.yaml`; adjust only the
   fixture path when needed.

5. During the run, capture both runtime and UI evidence:

- transcript final answer;
- timeline cards;
- trace drawer details;
- changed files/diff;
- validation commands;
- permission decisions;
- screenshots if the UI looks wrong.

6. Classify failures with the same buckets used by today's parser work:

- `tool_contract`
- `file_state`
- `bash_permission`
- `permission_recovery`
- `compaction_continuity`
- `llm_reasoning`
- `desktop_evidence`
- `desktop_ui`
- `desktop_runtime_bridge`

7. Fix the highest-impact defect, then rerun the same case before moving on.

## First Test Order

1. `core-simple-stale-edit`
2. `core-permission-rejection-recovery`
3. `core-rust-multi-file-refactor`
4. `core-long-output-artifact`
5. `code-change-verification-repair-loop`
6. `desktop-ui-smoke-polish`
7. `file_edit_rewind`
8. `bash_background_task`
9. `compaction_boundary`

Leave `subagent_worktree_worker` and `mcp_auth_repair` until the desktop app has
clearer support/fixtures for those surfaces.

## Completion Criteria

This test phase is complete when:

- all six release dogfood cases have desktop-run evidence;
- every failed or blocked case has an owner category and next fix;
- at least one permission denial/recovery case is verified through the desktop
  transcript permission card;
- at least one long-output case proves transcript collapse and trace access;
- at least one multi-file code change proves file/diff/validation cards are
  readable in the desktop app;
- `scripts/release-dogfood-gate.sh quick` remains green after fixes;
- the full CLI `agent-run` is used as a comparison after desktop issues are
  captured, not as a replacement for desktop dogfood.

## Dogfood Log

### 2026-05-23 - `core-simple-stale-edit`

Fixture:
`target/live-evals/desktop-dogfood-20260523-173242/core-simple-stale-edit/worktree`

Findings:

- `provider_health`: MiniMax was first quota-limited (`usage limit exceeded
  2056`), then passed health after recharge with `MiniMax-M2.7`.
- `desktop_runtime_bridge`: fixed selected-project runtime binding. Before the
  fix, the UI showed the dogfood worktree while the runtime tools executed in
  `/Users/georgexu/Desktop/rust-agent`.
- `desktop_packaging`: native automation can attach to the old bundle when dev
  and release apps share the same bundle id/name. Full `tauri build` is the
  reliable way to refresh the `.app`; manually copying the binary into the
  bundle produced a blank window.
- `provider_setup`: macOS GUI launches may not inherit shell provider env vars;
  dogfood used `launchctl setenv MINIMAX_API_KEY ...` as a temporary workaround.
- `bash_permission`: fixed readonly search classification. `grep ... 2>/dev/null
  | head` was incorrectly classified as `file_mutation`, causing a permission
  wait, duplicate failed bash cards, and repeated-tool stop.

Current status:

- Worktree-relative `file_read` is verified through the desktop transcript.
- After the bash classifier fix, the first turn no longer stalls on read-only
  grep, but MiniMax only read `settings.py` and asked for the target value
  instead of reading `test_settings.py`.
- After providing `10`, the follow-up still did not edit the file. The timeline
  showed repeated permission-denied bash mutation attempts and then
  `[Stopped noisy retries after repeated failures: file_edit]`.

Next owner:

- `permission_recovery`: mutation permission requests need a clearer active
  approval path in the desktop transcript and should not degrade into silent
  60s failed bash cards.
- `llm_reasoning`: the model should read the test file or infer the target from
  the validation before asking a redundant value question.

## Do Not Do

- Do not switch back to CLI-only testing for this phase unless the desktop app
  itself blocks the run; if it blocks, record that as a desktop defect.
- Do not expand the scenario set before the first six cases produce useful
  evidence.
- Do not mark UI problems as cosmetic if they hide validation, permissions,
  diffs, trace evidence, or session state.
- Do not claim Claude/Codex parity from these local desktop runs. External
  baseline capture still belongs to
  `docs/EXTERNAL_BASELINE_TEST_PLAN_2026-05-22.md`.

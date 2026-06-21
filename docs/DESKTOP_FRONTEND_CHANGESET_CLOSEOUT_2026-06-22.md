# Desktop Frontend Changeset Closeout - 2026-06-22

Status: Current worktree map, not final completion claim.

This note exists because the desktop frontend push has become a broad dirty
worktree. It records the real change boundaries, validation evidence, and
remaining closeout gates so the next commit/review step can stay deliberate.
Requirement-level completion evidence is tracked in
`docs/DESKTOP_FRONTEND_COMPLETION_AUDIT_2026-06-22.md`.

## Current Summary

- The desktop product direction is still the same: keep Direct Agent Mode and
  LabRun Mode as parallel desktop workflows, with shared runtime APIs and hard
  Rust-side boundaries.
- The current worktree includes multiple logical slices: React workbench UI,
  desktop runtime API cleanup, Tauri command/state cleanup, native smoke
  harnesses, LabRun/runtime backend support, and status docs.
- The frontend is usable enough for continued dogfood based on browser smoke,
  build, Rust/Tauri checks, and current native packaged smoke evidence.
- This is not yet a committed closeout. The current dirty tree now has a fresh
  native packaged smoke pass and a release-candidate dogfood pass, but still
  needs a clean commit boundary before the work should be treated as finished.

## Suggested Commit Scopes

### 1. Desktop Workbench UI And Daily-Use Polish

Primary files:

- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/app/components/SessionHeader.tsx`
- `apps/desktop/src/app/components/WorkspaceTopbar.tsx`
- `apps/desktop/src/app/components/StartupStateCard.tsx`
- `apps/desktop/src/app/components/RuntimeInspectorSurfaces.tsx`
- `apps/desktop/src/app/components/InspectorPanel.tsx`
- `apps/desktop/src/app/components/InspectorPrimitives.tsx`
- `apps/desktop/src/app/components/Composer.tsx`
- `apps/desktop/src/app/components/CommandPalette.tsx`
- `apps/desktop/src/app/components/DeleteSessionDialog.tsx`
- `apps/desktop/src/app/components/ExportNoticeBanner.tsx`
- `apps/desktop/src/app/components/Transcript.tsx`
- `apps/desktop/src/app/components/*Drawer.tsx`
- `apps/desktop/src/app/state/*`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src/app/runEventPresentation.ts`
- `apps/desktop/src/app/types.ts`
- `apps/desktop/src/styles/global.css`
- `apps/desktop/src/styles/parts/*.css`
- `apps/desktop/tests/desktop-ui-smoke.spec.ts`
- `apps/desktop/tests/run-event-state.spec.ts`

Implemented behavior:

- Three-pane workbench shell with session header, persistent runtime inspector,
  Direct Agent / LabRun mode entry, and narrow-screen drawer fallback.
- Runtime inspector surfaces for Context, Files, Execution, Subagents, LabRun,
  and Diagnostics.
- Composer improvements: slash commands, prompt history, context attachment
  controls, screenshot unavailable note instead of dead disabled action, and
  global composer focus shortcut.
- Transcript run summaries with validation, diff/file changes, permissions,
  failures, tool evidence, final text, and trace links.
- Focus and keyboard polish for command palette restoration, destructive
  session deletion, export feedback, and environment popover close behavior.
- Source-size cleanup: `App.tsx` is under the 1500-line project ceiling.

Current maintenance note:

- `InspectorPanel.tsx` is under the ceiling but still large. The next cleanup
  should split inspector tab bodies only after the current behavior is committed
  and frozen by smoke tests.

### 2. Desktop Runtime API And Browser Fixture Split

Primary files:

- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/runtime/desktopTypes.ts`
- `apps/desktop/src/runtime/desktopGoalApi.ts`
- `apps/desktop/src/runtime/desktopPreview.ts`

Implemented behavior:

- Shared desktop DTOs and API types moved out of the monolithic runtime API
  file while preserving the public import surface.
- Goal helpers and browser-preview fixtures moved into focused modules.
- Preview fixtures now cover run events, permission answers, manual compaction,
  LabRun report/artifact rows, file preview, and listener fanout without
  mixing fixture logic into the real Tauri API boundary.

Current maintenance note:

- `desktopApi.ts` is back under the source-size ceiling but still central.
  Future API additions should go into focused runtime modules first.

### 3. Tauri Command, DTO, And State Cleanup

Primary files:

- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/desktop_types.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `apps/desktop/src-tauri/src/desktop_state/native_smoke.rs`
- `apps/desktop/src-tauri/src/desktop_context.rs`
- `apps/desktop/src-tauri/src/health_commands.rs`
- `apps/desktop/src-tauri/src/session_commands.rs`
- `apps/desktop/src-tauri/src/preview_commands.rs`
- `apps/desktop/src-tauri/src/goal_commands.rs`
- `apps/desktop/src-tauri/src/revert_commands.rs`
- `apps/desktop/src-tauri/src/tests.rs`

Implemented behavior:

- Command handlers are split by domain while retaining existing command names.
- Shared settings, diagnostics, provider, session, and response DTOs now live in
  `desktop_types.rs`.
- Native smoke fixture preparation and injected smoke scripts moved under
  `desktop_state/native_smoke.rs`.
- Active session recovery hardening clears stale desktop session ids before
  settings/runtime initialization can surface `session not found`.
- Guarded preview/read APIs stay Rust-side and reject out-of-project or
  unsupported reads.

Current maintenance note:

- `lib.rs` is under the 1500-line ceiling but still close enough that new Tauri
  commands should start in focused modules.

### 4. Native Smoke And Release Dogfood Harness

Primary files:

- `scripts/desktop-native-smoke.sh`
- `scripts/desktop-release-dogfood.sh`

Implemented behavior:

- Native smoke now covers real Tauri desktop flows rather than only browser
  preview behavior.
- The script supports live provider, restart, Lab recovery, multi-tool, soak,
  and screenshot/no-screenshot variants.
- Smoke summary logs now include key diagnostic lines so a short summary file
  remains useful without opening the full app log.
- Release dogfood script provides one entrypoint for build, smoke, restart,
  multi-tool, soak, and artifact summary checks.

Current maintenance note:

- The post-closeout native packaged restart/LabRun recovery gate passed after
  the latest frontend focus/export/environment changes. Longer real-provider
  dogfood is still useful before a release candidate, but the basic packaged
  desktop gate is no longer missing.

### 5. LabRun And Runtime Backend Support

Primary files:

- `src/lab/commands.rs`
- `src/lab/draft.rs`
- `src/lab/orchestrator.rs`
- `src/desktop_runtime/mod.rs`
- `src/bootstrap.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/conversation_loop/tests.rs`
- `src/agent/memory.rs`
- `src/tools/rewind_tool/mod.rs`

Implemented behavior:

- Desktop-facing LabRun commands expose proposal, intake, draft, project
  controls, reports, artifacts, evidence refs, and professor side-channel
  surfaces without letting the frontend command internal agents directly.
- LabRun graduate execution policy was moved away from provider-name
  pre-certification and toward task evidence, postdoc review, and runtime
  proof/cleanup boundaries.
- Runtime support was adjusted for desktop startup/state and LabRun surfaces
  while preserving the project rule that semantic judgment belongs to the LLM,
  not to hidden framework intelligence.

Current maintenance note:

- LabRun can keep dogfooding, but release readiness still depends on longer
  multi-cycle runs and review/cleanup evidence.

### 6. Documentation

Primary files:

- `docs/PROJECT_STATUS.md`
- `docs/DESKTOP_FRONTEND_PRODUCT_PLAN_2026-06-21.md`
- `docs/LAB_AGENT_WORKFLOW_PLAN_2026-06-18.md`
- `docs/LAB_GRADUATE_EXECUTION_POLICY_DISCUSSION_2026-06-21.md`
- `docs/DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md`

Implemented behavior:

- Project status now records desktop workbench progress, native smoke evidence,
  source cleanup, and remaining release-readiness gaps.
- LabRun docs record the updated graduate execution policy discussion:
  provider-neutral execution, postdoc review responsibility, branch cleanup, and
  professor steering triggers.
- This closeout note records the dirty-tree map so the final commit can be
  scoped and reviewed without relying on memory.

## Validation Evidence Already Seen

Recent local checks from this desktop frontend push:

- `corepack pnpm --dir apps/desktop build`
- `corepack pnpm --dir apps/desktop test:ui-smoke`
  - Latest full browser smoke observed: 38 passed.
- `corepack pnpm --dir apps/desktop exec playwright test tests/desktop-ui-smoke.spec.ts --grep "desktop layout renders core controls"`
- `cargo check -q`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q`
  - Latest desktop Tauri Rust test run observed: 38 passed.
- `bash -n scripts/desktop-native-smoke.sh`
- `scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 180 --no-screenshot`
- `scripts/desktop-native-smoke.sh --skip-build --lab-recovery-check --restart-check --timeout 180 --no-screenshot`
- `scripts/desktop-native-smoke.sh --restart-check --lab-recovery-check --timeout 240 --no-screenshot`
  - Latest post-closeout packaged native smoke passed with logs:
    `apps/desktop/test-artifacts/native-lab-recovery-smoke.log` and
    `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`.
- `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --restart-check --timeout 180 --no-screenshot`
- `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --multi-tool-check --timeout 240 --no-screenshot`
- `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --restart-check --timeout 480 --no-screenshot`
- `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 1`
  - Latest release-candidate dogfood passed against the current packaged app.
    It ran DeepSeek extended soak + restart, MiniMax extended soak + restart,
    and LabRun recovery + restart. Summary:
    `apps/desktop/test-artifacts/desktop-release-dogfood.log`.
  - Current evidence logs:
    `apps/desktop/test-artifacts/native-extended-soak-deepseek-app-desktop.log`,
    `apps/desktop/test-artifacts/native-extended-soak-minimax-app-desktop.log`,
    and `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`.
- `git diff --check`

These checks justify continued dogfood. They do not replace release-owner
judgment about whether to run longer real-provider dogfood again before a
release candidate.

## Required Before Calling This Done

- Fresh full desktop browser smoke after the closeout doc/status update:
  complete.
- Fresh packaged native smoke after the latest frontend polish:
  complete with `--restart-check --lab-recovery-check`.
- Narrow Rust/Tauri checks for touched backend files:
  complete for `cargo check -q` and
  `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q`.
- Release-candidate real-provider dogfood:
  complete for `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 1`.
- Decide whether to commit as one broad desktop milestone or split into the
  commit scopes above.
- Optional after commit-boundary freeze: rerun the longer real-provider release
  dogfood suite if any staged/commit-splitting step changes runtime code or the
  packaged app.
- Re-check source file sizes after any final edit. `InspectorPanel.tsx`,
  `Composer.tsx`, `desktopApi.ts`, and `lib.rs` should not receive more mixed
  responsibility in this slice.

## Recommendation

Stop adding frontend polish in this branch unless a final smoke exposes a real
daily-use blocker. The next high-value action is either:

1. commit this changeset with the validation evidence above;
2. or split into the scopes above and rerun the long dogfood suite only for any
   split that changes runtime code or the packaged app.

For product readiness, the honest status is: desktop is strong enough for
continued daily dogfood, but not yet release-ready until the frozen dirty tree
has a clean commit boundary.

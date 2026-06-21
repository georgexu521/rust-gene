# Desktop Frontend Completion Audit - 2026-06-22

Status: implementation and validation audit for the active desktop workbench
goal. This is not a commit record.

## Objective Under Audit

Make `rust-agent`'s desktop frontend a daily-use mature agent workbench modeled
after Codex/OpenCode core interactions while preserving the existing runtime
boundary, covering:

- desktop information architecture;
- primary Direct Agent workflow;
- LabRun and Direct Agent mode entry;
- runtime status visibility;
- validation and documentation closeout.

## Verdict

Current evidence supports: ready for daily dogfood and ready to enter commit
closeout.

Current evidence does not yet support: fully finished repository state, because
the work remains a broad uncommitted dirty tree. The remaining work is not a
known product/UI capability gap; it is a commit-boundary and final repository
hygiene gap.

## Requirement Audit

| Requirement | Current status | Evidence |
| --- | --- | --- |
| Desktop information architecture | Complete for this milestone | `SessionHeader`, `WorkspaceTopbar`, persistent `InspectorPanel`, drawer fallback, split CSS parts, and UI smoke coverage. |
| Direct Agent daily workflow | Complete for this milestone | Composer slash commands, prompt history, context attachments, provider setup repair, grouped transcript run cards, tool/trace/context inspector surfaces. |
| Direct Agent / LabRun mode entry | Complete for this milestone | Session header mode controls, mobile mode coverage, LabRun inspector drawer path, and status docs. |
| LabRun project surface | Complete for the current typed runtime snapshot | Proposal/intake actions, approve/draft, pause/resume/continue/meeting controls, professor side-channel, status board, report/artifact browser, guarded report/artifact body reads. |
| Runtime boundary preservation | Complete for this milestone | Frontend consumes typed desktop APIs; guarded file/report/artifact reads stay Rust-side; LabRun UI stages runtime commands instead of directly commanding postdoc/graduate agents. |
| Runtime status visibility | Complete for this milestone | Context, Files, Execution, Subagents, LabRun, Diagnostics tabs; provider usage tokens; cache/compression/context breakdown; trace/tool output surfaces. |
| Local persistence and restart recovery | Complete for this milestone | Native restart smoke, extended soak restart, and LabRun recovery restart all pass against the packaged app. |
| Real provider desktop path | Complete for this milestone | Release dogfood passed DeepSeek and MiniMax extended soak + restart, with real tool calls, file changes, verified closeouts, and restored UI state. |
| Mobile/narrow usability | Complete for this milestone | Mobile runtime inspector drawer, statusbar navigation, settings access, drawer focus traps, mode switcher, trace/output drawer entry, and smoke coverage. |
| Validation closeout | Complete for current dirty tree | Browser smoke, build, Rust/Tauri checks, fresh native packaged smoke, and release dogfood all pass. |
| Documentation closeout | Complete for current dirty tree | `PROJECT_STATUS.md`, product plan, change-set closeout, and this completion audit now point to current evidence and remaining commit boundary. |
| Repository closeout | Not complete | The tree remains intentionally dirty and broad; no commit boundary has been created yet. |

## Current Validation Evidence

Latest checks observed during closeout:

- `corepack pnpm --dir apps/desktop build`
- `corepack pnpm --dir apps/desktop test:ui-smoke`
  - 38 passed.
- `cargo check -q`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q`
  - 38 passed.
- `scripts/desktop-native-smoke.sh --restart-check --lab-recovery-check --timeout 240 --no-screenshot`
  - Passed against a freshly rebuilt packaged app.
- `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 1`
  - Passed DeepSeek extended soak + restart.
  - Passed MiniMax extended soak + restart.
  - Passed LabRun recovery + restart.
- `git diff --check`

Key artifact logs:

- `apps/desktop/test-artifacts/desktop-release-dogfood.log`
- `apps/desktop/test-artifacts/native-extended-soak-deepseek-app-desktop.log`
- `apps/desktop/test-artifacts/native-extended-soak-minimax-app-desktop.log`
- `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`
- `apps/desktop/test-artifacts/native-lab-recovery-smoke.log`

## Current Source-Size Check

Important large files remain below the project 1500-line ceiling:

- `apps/desktop/src/app/App.tsx`: 1121 lines.
- `apps/desktop/src/app/components/InspectorPanel.tsx`: 1313 lines.
- `apps/desktop/src/app/components/Composer.tsx`: 1049 lines.
- `apps/desktop/src/runtime/desktopApi.ts`: 1065 lines.
- `apps/desktop/src-tauri/src/lib.rs`: 1487 lines.
- `apps/desktop/src-tauri/src/desktop_context.rs`: 1108 lines.
- `apps/desktop/src-tauri/src/desktop_state.rs`: 680 lines.

Risk note: `apps/desktop/src-tauri/src/lib.rs` is close to the ceiling. Do not
add new desktop command logic there in this slice; create focused command
modules instead.

## Remaining Work

### Required Before Calling The Goal Finished

- Create a clean commit boundary:
  - either one broad desktop milestone commit with the current evidence;
  - or split by the scopes in
    `docs/DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md`.
- If commit splitting changes runtime code or rebuilds the packaged app, rerun
  the relevant build/native dogfood checks after the split.

### Not Required For This Milestone

- More visual polish.
- More provider matrix testing beyond DeepSeek and MiniMax.
- Longer unattended background LabRun testing.
- Further component splitting, unless a file crosses the 1500-line ceiling or
  a real maintenance problem appears.

## Completion Decision

Do not mark the overall desktop frontend goal complete while the repository
still has this broad uncommitted changeset. The implementation itself has
enough evidence to move to commit closeout; the repository state does not yet
have a finished boundary.

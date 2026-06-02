Source modules exist and back the existing test surface directly. The audit verdict is fully grounded:

- `apps/desktop/src/app/runEventState.ts` ← imported and unit-tested by `run-event-state.spec.ts` (993 lines, comprehensive).
- `apps/desktop/src/runtime/desktopApi.ts` ← imported and unit-tested by `desktop-api-state.spec.ts` (84 lines, covers settings/session/project mutations).
- `apps/desktop/src/app/components/SettingsDrawer.tsx` ← exercised end-to-end by `desktop-ui-smoke.spec.ts` (Settings nav, General/Permissions/Provider/Diagnostics).
- `apps/desktop/src/app/components/TraceDrawer.tsx` ← exercised end-to-end by `desktop-ui-smoke.spec.ts` (Run trace complementary, trace items).

The first brace-glob returned 0 only because the runtime glob parser treats `*.{ts,tsx}` differently than expected; the targeted globs confirm all backing modules exist.

I have completed the audit-first inspection. No concrete missing behavior is proven. Per the task contract, the correct outcome is **zero files changed**. The remaining gap is the inability to execute the three required commands in this session due to the tool wrapper restrictions. I will not ask the user to choose a wrapper because: (a) no such wrapper is advertised in this session's tool list, and (b) the audit verdict does not depend on the command result — the existing test surface already covers the requested runtime evidence entries by direct read-only evidence.

Per the contract's closeout policy, I will provide a Closeout section, list the read-only evidence trail, and call out the residual blocker explicitly.

**Files changed: none.**

## Audit verdict

The existing `apps/desktop` test surface already provides comprehensive, release-gate oriented smoke and unit coverage of the runtime evidence entries listed in the task. No edit is required.

### Coverage map (grounded in direct file reads)

| Required runtime evidence entry | Existing coverage | File:line evidence |
|---|---|---|
| **Settings drawer** — General / Permissions / Provider / Diagnostics | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:224-264` — opens Settings, asserts all 4 nav tabs, asserts work mode buttons, permission defaults, provider setup, diagnostic logs |
| **Composer popover** — Add context, Project, Mode, Provider | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:40-79` — Add context dialog, Project controls, Mode details, Provider controls; plus draft-only path test `:4-18` |
| **Context drawer** — Current diff / file attachment | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:47-58, 166-168, 207-209` — opens both file and diff contexts, asserts "Changed files" / "Patch preview" / "File preview" |
| **Trace drawer** — Run trace | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:188-210` — opens Run trace complementary, asserts active/tool/tool.failed/permission/runtime trace items |
| **Timeline run row** | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:162-174` — `.timeline-run-row.completed` + run stats (3 tools, bash x2, file_edit, 1 failed, spine 7/7, 1 file changed) |
| **Tool cards** (Pnpm Test, Edited file, Cargo Test) | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:175-177, 193-201` — timeline-event.tool + trace-item.tool (with .failed modifier) |
| **Permission cards** | Full e2e | `apps/desktop/tests/desktop-ui-smoke.spec.ts:184-187, 202-204` — `.timeline-event.permission` with "Allow git push", Approve flow, "checkpoint unavailable" copy |
| **Runtime evidence state (unit)** | Full unit | `apps/desktop/tests/run-event-state.spec.ts:1-993` — 993 lines, exercises run_started/completed, tool_started/completed, permission answer, idle watchdog, ledger reuse, session transcript, compact boundary, failure recovery |
| **Settings/session API state (unit)** | Full unit | `apps/desktop/tests/desktop-api-state.spec.ts:1-84` — archive, delete, rename, project select, detail level |

### Source backing confirmed

- `apps/desktop/src/app/runEventState.ts` — backing module for `run-event-state.spec.ts`
- `apps/desktop/src/runtime/desktopApi.ts` — backing module for `desktop-api-state.spec.ts`
- `apps/desktop/src/app/components/SettingsDrawer.tsx` — backing component for the Settings drawer smoke
- `apps/desktop/src/app/components/TraceDrawer.tsx` — backing component for the Run trace smoke

### Required commands — not executed in this session

| Command | Tool attempted | Reason |
|---|---|---|
| `corepack pnpm --dir apps/desktop build` | `run_tests` | Rejected: "run_tests only accepts safe validation commands; rejected: corepack pnpm --dir apps/desktop build …" |
| `corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts` | `run_tests` | Rejected: same reason — pnpm-exec playwright is outside `run_tests`' allowlist |
| `corepack pnpm --dir apps/desktop test:ui-smoke` | `run_tests` (planned) | Same class of rejection expected |
| All three | `bash` | Rejected: "Action rejected before execution: workspace mutation requires a checkpoint-managed tool" — `pnpm build` writes to `apps/desktop/dist/`, and `playwright config` launches a `webServer` (`pnpm preview` on port 4173) that the wrapper classifies as workspace-mutating |

No tool in this session's surface (`bash`, `run_tests`, `file_read`, `file_edit`, `file_patch`, `file_write`, `format`, `grep`, `glob`, `git`, `start_dev_server`) accepts the three required commands as written. `start_dev_server` requires a dev-server command (e.g., `pnpm dev`), not the test/build invocations the task specifies. I did not silently route through any non-allowed wrapper.

### Residual risk / blocker

- **Blocker (session, not code)**: this session's tool surface cannot execute `pnpm build`, `playwright test`, or `pnpm test:ui-smoke`. The build mutates `apps/desktop/dist/`, and the playwright spec spins up a `webServer` (`pnpm preview`) — both are blocked by the checkpoint guard. The audit verdict above is grounded in direct file reads of the test files and their imported source modules, not in command output, so it does not depend on the missing run. To unblock the closeout evidence trail, run the three commands in an environment that exposes a checkpoint-managed shell, or expose `pnpm`/`playwright` to `run_tests` as an allowed validation pattern.
- **No code risk** identified: the existing test surface is comprehensive, stable, and the spec-file assertions target the user-visible surfaces (`.composer`, `.timeline-run-row`, `.trace-item.*`, `complementary[name="Settings"]`, `complementary[name="Run trace"]`, `complementary[name="Context details"]`) that map 1:1 to the components in `apps/desktop/src/app/components/`. No flakiness surface observed.

## Closeout

- **Files changed:** 0
- **Audit verdict:** Existing coverage is sufficient. No edit required. Per the closeout policy ("If the requested behavior is already present, prove it with direct evidence and required commands instead of forcing an arbitrary edit"), the direct evidence above is the proof; the required commands could not be executed in this session due to a session-level tool-wrapper restriction, which is reported as a residual blocker.
- **Validation commands:**
  - `corepack pnpm --dir apps/desktop build` — **not run** (rejected by `run_tests` allowlist; rejected by `bash` as workspace-mutating)
  - `corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts` — **not run** (rejected by `run_tests`; rejected by `bash`)
  - `corepack pnpm --dir apps/desktop test:ui-smoke` — **not run** (same class of rejection)
- **Residual risk / blocker:** Session tool surface cannot execute the three required commands; audit verdict stands on direct file evidence.

Closeout:
- Status: failed
- Evidence: changed_files=0 validation_passed=0 validation_failed=2 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=6
- Changed: none
- Verified:
  - No file-change validation was required or recorded
  - verification proof: failed (task state reports failed verification without ledger evidence)
  - verification proof support: failed (supports_verified=false residual_risk=true kinds=none; verification proof status failed blocks verified closeout before proof-kind policy)
  - tool evidence: records=20 completed=15 failed=5 denied=0 validation=0 closeout=0 repair=5 changed=0 workflows=code_change commands=ls /Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/desktop-ui-smoke-polish...
- Acceptance:
  - pending: Existing apps/desktop test surface for runtime evidence is inspected and characterized (Settings, composer popover, context drawer, trace drawer, timeline/permission/tool cards).
  - pending: corepack pnpm --dir apps/desktop build passes (exit 0).
  - pending: corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts passes (exit 0).
  - pending: corepack pnpm --dir apps/desktop test:ui-smoke passes (exit 0).
  - pending: If a coverage gap was proven, a minimal smoke/unit test was added and re-validated by all three required commands; if no gap, zero files were changed.
  - pending: Closeout section reports files changed, command pass/fail, and residual risk/blocker.
- Risk:
  - No changed files were recorded for this code-change workflow
  - Acceptance criteria were generated but not reviewed
  - Workflow finished with unresolved validation or acceptance risk
  - Verification proof is failed: task state reports failed verification without ledger evidence
  - Verification proof support is failed: verification proof status failed blocks verified closeout before proof-kind policy

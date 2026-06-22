# Next Phase Product Ecosystem Gap Plan
Status: Completed

> 2026-06-09 → 2026-06-10
> All 6 phases (A-F) implemented. Latest commits: `9d2935d2`
> surface gaps without weakening Priority Agent's local, verifiable runtime.

## 0. Summary

The latest opencode-facing work narrowed core programming-agent gaps around edit
recovery, shell parsing observations, and long-output hints. The remaining gap
is now less about "can the agent edit code" and more about product maturity:

- provider setup and onboarding should be guided, persisted, and inspectable;
- LSP should become a productized diagnostic surface, not just an available
  manager;
- sessions need first-class export/share/resume semantics across TUI and
  desktop;
- multi-session and agent-profile workflows should become visible daily
  workflows;
- desktop/IDE context should carry current file and selection into the runtime;
- polish should be measured through product-readiness checks before broader
  soak tests.

This plan deliberately keeps the product direction narrow: Priority Agent should
stay gex's local, personal, evidence-driven coding partner. Do not copy broad
opencode defaults when they conflict with checkpoint, permission, validation, or
privacy boundaries.

## 0.1 Implementation Review Update

Status after the 2026-06-09 implementation follow-up:

- Phase A is partially landed: `provider_catalog` and credential status power
  `/connect` and `/credentials`; provider catalog env aliases now match the
  runtime registry for Kimi/Moonshot and GLM/ZAI/ZHIPUAI/BIGMODEL.
- Phase B is partially landed: `LspConfig` is part of
  `AppConfig`, bootstrap respects `lsp.enabled`, detected servers can receive
  command/args/env overrides, and `/lsp stop|restart <name>` calls the manager
  instead of returning a no-op hint. `file_edit`, `file_write`, and `file_patch`
  now expose optional LSP diagnostic metadata/delta when initialized servers are
  available.
- Phase C local export/share is landed across TUI and desktop: `/export
  [json|md] [full|redacted|summary]`, `/share local ...`, and desktop redacted
  markdown export use `session_store::export` instead of shelling out. Export
  payloads now include messages, changed-file hints from session events, recent
  reverts, diagnostics records, and tool stats.
- Phase D is partially landed: `/agent list`, `/agent <profile>`, `/agent
  switch <mode>`, and `/agent run <profile> <prompt>` are wired. Desktop agent
  picker/job projection remains future work.
- Phase E is partially landed in desktop: file/diff context cards can be
  attached and injected with bounded provenance, and file contexts now support
  line-range/selection metadata for IDE handoff. CLI helper and actual IDE
  extension remain future work.
- Phase F is partially landed: `/product-ready` exposes readiness checks and
  desktop diagnostics include the same product-readiness DTO. `/doctor product`
  alias and richer provider-health recency remain future work.

Remaining implementation gaps should now focus on desktop agent selection/job
projection, CLI/IDE context handoff commands, richer structured diagnostic
mirroring in session events, and broad product soak/polish.

## 1. External Reference Points

Current opencode product capabilities to use as comparison anchors:

- Providers: `/connect` stores credentials and provider config supports 75+
  providers and local models through AI SDK / Models.dev.
  Source: <https://opencode.ai/docs/providers>
- LSP: optional LSP integration starts servers based on file extensions and uses
  diagnostics as agent feedback, while warning that LSP can be slower or stale
  than direct validation commands.
  Source: <https://opencode.ai/docs/lsp/>
- Agents: built-in primary agents and subagents, per-agent model/tool/permission
  controls, and Markdown agent definitions.
  Source: <https://opencode.ai/docs/agents/>
- IDE: split-terminal launch, new-session launch, current selection/tab context,
  and file-reference shortcuts.
  Source: <https://opencode.ai/docs/ide/>
- Share: manual/auto/disabled share modes with privacy warnings.
  Source: <https://opencode.ai/docs/share/>

These references are product targets, not runtime-policy targets.

## 2. Current Priority Agent State

### Provider / onboarding

Current code:

- `src/services/api/provider.rs` has deterministic built-in provider env specs
  for MiniMax, Kimi Code, DeepSeek, GLM, Kimi, and OpenAI.
- `src/tui/app/palette.rs` exposes provider/model pickers from configured env.
- `src/tui/app/slash_commands.rs` supports `/provider`, `/provider status
  --json`, and `/model`.
- `src/services/config.rs` can load/save redacted config, but provider secrets
  are still mostly env-driven.
- `apps/desktop/src-tauri/src/desktop_state.rs` and
  `apps/desktop/src/app/components/SettingsDrawer.tsx` show provider status and
  setup guidance.

Gap:

- There is no real `/connect` equivalent that writes a secret safely or produces
  a durable provider profile.
- `/login` is local state only and does not configure a provider.
- Provider model catalogs are hard-coded in multiple UI paths instead of coming
  from one registry DTO.
- Onboarding still tells users to export env vars rather than walking them
  through a setup flow.

### LSP

Current code:

- `src/engine/lsp.rs` implements a stdio LSP client, diagnostics cache, hover,
  definition, references, document symbols, workspace symbols, and manager
  registration.
- `LspManager::detect_servers()` detects Rust, TypeScript, Go, and Python
  projects.
- `src/tui/slash_handler/agents/lsp.rs` exposes `/lsp list`, `restart`, and
  `stop`; stop/restart now call manager methods.
- LSP manager is injected into TUI state, and file mutation tools expose
  optional diagnostic metadata when initialized LSP clients are available.

Gap:

- LSP config exists in `AppConfig`, but richer extension-based routing from
  per-server config is still incomplete.
- No file-extension-driven lazy start from file read/edit.
- LSP diagnostic metadata is still opportunistic: it only uses already
  initialized clients and should be mirrored more directly into durable session
  events.
- No safe auto-download policy; external server installation should remain
  opt-in for this project.

### Sessions / share / export

Current code:

- `src/session_store/` persists sessions, messages, parts, events, traces,
  compaction boundaries, reverts, todos, agent artifacts, and FTS search.
- `src/tui/session_manager.rs` supports session export and resume selection.
- Desktop has recent sessions, search, resume, rename, archive, delete, and
  persisted active session settings.
- TUI `/export` and `/share local` write local JSON/Markdown exports through
  `session_store::export` with full/redacted/summary privacy tiers.

Gap:

- Public/network share is intentionally still disabled; current share means
  local private export only.
- Desktop redacted markdown export is wired to the shared export path, but
  open-export-folder/copy-path controls are still missing.
- Export payload now includes changed-file hints, diagnostics record slots,
  reverts, and tool stats; it still needs richer structured diagnostic mirroring
  from session events.

### Multi-session / agents

Current code:

- `src/agent/profiles.rs` has rich agent profile definitions: context mode,
  permission mode, risk policy, output contract, model policy, memory policy,
  timeout, and max turns.
- `src/tools/agent_tool/` supports subagent execution and artifact/result
  state.
- `src/engine/worktree` and session artifacts support isolated work.
- TUI panels and slash commands can list agent definitions.

Gap:

- Agent profiles are not presented as daily primary/subagent modes in a simple
  way.
- There is no opencode-like Build/Plan/Explore/Review product surface with
  clear permissions and model policy.
- Multi-session work is stored, but not surfaced as "parallel agents on the same
  project" with visible progress, branch/worktree, and proof status.

### Desktop / IDE context

Current code:

- Desktop runs through the shared `StreamingQueryEngine`.
- `apps/desktop/src/runtime/desktopApi.ts` has run contexts, current diff/file
  context detail, workbench snapshots, symbol index snapshots, and tool-output
  paging.
- `apps/desktop/src/app/components/Composer.tsx` can send message contexts.

Gap:

- There is no IDE extension bridge.
- Desktop context is project/workbench oriented, not "current editor file,
  current selection, active tab" oriented.
- File references are not a first-class shortcut in TUI/desktop.

### Product polish / readiness

Current code:

- `/doctor`, `/diagnostic`, provider health, config doctor, product-readiness
  formatting, desktop diagnostics, and run reports all exist.
- `docs/archive/REAL_TASK_SOAK_TEST_PLAN_2026-06-07.md` covers larger real-task soak
  testing, but the user explicitly wants to wait on tests and continue product
  development first.

Gap:

- Product readiness checks are not yet organized around install/setup/session
  restore/provider/LSP/export/desktop user flows.
- Many UX gaps are known but not converted into small product slices.

## 3. Development Principles

1. Keep provider secrets local. Prefer keychain or env-file export guidance over
   writing plaintext secrets into normal config.
2. Keep LSP optional and diagnostic-first. Validation commands remain the
   authoritative proof path.
3. Treat share as private export first. Do not upload sessions or create public
   links without a separate privacy review.
4. Make agent profiles visible before adding more orchestration. Better daily UI
   beats another hidden abstraction.
5. Keep tests narrow during implementation. Delay soak/eval expansion until the
   product flows exist.

## 4. Phase Plan

### Phase A: Provider Setup And Onboarding

Goal: make first-run and provider switching feel product-ready without changing
the provider request boundary.

Code changes:

- Add `src/services/api/provider_catalog.rs`.
  - Centralize provider id, label, env vars, default base URL, default model,
    supported model list, docs URL, and setup hint.
  - Re-export or replace duplicated lists from `provider.rs`,
    `desktop_state.rs`, and `tui/app/palette.rs`.
- Add `src/services/api/credentials.rs`.
  - Phase A.1: read-only status and shell-profile export-line generation.
  - Phase A.2: optional macOS Keychain integration behind a feature flag or
    explicit command.
- Add `/connect <provider>` in TUI.
  - Show required key env vars, configured status, default model, and exact
    shell-profile line.
  - Do not echo the secret back.
  - Offer "open shell profile" / "open settings folder" actions in desktop.
- Update desktop Settings provider tab.
  - Show configured/unconfigured providers from the same catalog DTO.
  - Add copyable export line and model picker derived from catalog.
  - Keep actual provider switching through existing runtime setter.

Candidate files:

- `src/services/api/provider.rs`
- `src/services/api/provider_catalog.rs`
- `src/services/api/credentials.rs`
- `src/tui/app/palette.rs`
- `src/tui/app/slash_commands.rs`
- `src/tui/slash_handler/agents/auth_status.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `apps/desktop/src/app/components/SettingsDrawer.tsx`
- `apps/desktop/src/runtime/desktopApi.ts`

Acceptance:

- `/provider list`, desktop provider options, and provider setup diagnostics
  all use one catalog.
- `/connect minimax` gives a concrete setup path without leaking secrets.
- Existing env-based providers keep working.

Narrow gates:

```bash
cargo test -q provider
cargo test -q config
cargo test -q tui --lib
```

### Phase B: LSP Productization

Goal: turn the existing LSP manager into an optional diagnostics surface that
can help the model after edits without becoming a hard proof substitute.

Code changes:

- Add `LspConfig` to `src/services/config.rs`.
  - `lsp.enabled: bool`
  - `lsp.auto_detect: bool`
  - `lsp.disable_downloads: bool`
  - `lsp.servers.<id>.command/args/extensions/env/disabled`
- Extract `src/engine/lsp.rs` into a small module set if edits get large:
  - `client.rs`
  - `manager.rs`
  - `registry.rs`
  - `diagnostics.rs`
- Add an LSP registry with Rust, TypeScript, Go, Python, Bash, YAML, and JSON
  entries.
  - Only start servers when command is available.
  - No auto-install in this phase.
- Implement real `/lsp restart <name>` and `/lsp stop <name>`.
- Add file-extension routing:
  - `manager.client_for_path(path)`
  - `manager.sync_file_for_diagnostics(path, content)` or the existing
    file-tool diagnostic sync helper
  - `manager.diagnostics_for_path(path)`
- Attach optional diagnostics to file mutation metadata after `file_write`,
  `file_edit`, and `file_patch`.
  - Mark source as `lsp_optional`.
  - If LSP is unavailable or times out, record diagnostic status but do not
    fail the edit.

Candidate files:

- `src/engine/lsp.rs`
- `src/services/config.rs`
- `src/tui/slash_handler/agents/lsp.rs`
- `src/tools/file_tool/mod.rs`
- `src/tools/file_tool/diagnostics.rs`
- `src/tools/file_tool/state.rs`
- `apps/desktop/src/app/components/DiagnosticsPanel.tsx`

Acceptance:

- `lsp.enabled=false` is the default unless explicitly enabled.
- `/lsp list` shows configured and running status.
- `/lsp stop/restart` actually changes manager state.
- After editing a Rust file with LSP enabled, metadata can include diagnostic
  count and source.
- Failed or missing language server never weakens validation closeout.
- Current implementation note: `file_edit`, `file_write`, and `file_patch`
  expose `diagnostics`, `diagnostics_after`, and `diagnostics_delta`; this is
  still optional evidence and requires initialized LSP clients.

Narrow gates:

```bash
cargo test -q lsp
cargo test -q file_tool
cargo test -q config
```

### Phase C: Private Session Export And Share Semantics

Goal: make "share/export" honest, private, and useful before considering any
network share link.

Code changes:

- Add `src/session_store/export.rs`.
  - `SessionExportFormat::{Json, Markdown}`
  - `SessionExportPrivacy::{Full, Redacted, Summary}`
  - include session metadata, messages, tool summaries, changed files, reverts,
    diagnostics, and provider/runtime facts.
- Replace placeholder `/export [json|md]` bash echo path with direct store
  export.
- Rename or clarify `/share`:
  - `/share local [json|md]`
  - `/share status`
  - `/share disabled` remains default for network.
- Desktop:
  - add export action from session menu;
  - show generated path;
  - add "open export folder";
  - no public upload.

Candidate files:

- `src/session_store/export.rs`
- `src/session_store/mod.rs`
- `src/tui/session_manager.rs`
- `src/tui/slash_handler/session.rs`
- `src/tui/slash_handler/session/actions.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/app/components/Sidebar.tsx`

Acceptance:

- `/export json` and `/export md` write real session exports without shelling
  out.
- `/share local` is explicit about local/private export.
- Export includes redaction metadata.
- Desktop and TUI use the same export code path.
- Current implementation note: TUI `/export` and `/share local` plus desktop
  title-bar export use `session_store::export`; payloads include changed-file
  hints, reverts, diagnostics record slots, and tool stats. Open-export-folder
  and any public/network share mode remain future work.

Narrow gates:

```bash
cargo test -q session_store
cargo test -q session_manager
cargo test -q slash
```

### Phase D: Visible Agent Profiles And Multi-Session Workflows

Goal: expose existing agent-profile infrastructure as a usable daily workflow,
not just a hidden subagent mechanism.

Code changes:

- Add built-in product profiles:
  - `build`: full primary coding mode, normal permissions;
  - `plan`: read-only/ask mode, no writes unless user approves;
  - `explore`: read/search/LSP/symbol context, no edits;
  - `review`: diff-aware findings, no edits;
  - `verify`: run validation and summarize proof.
- Add `/agent list`, `/agent switch <name>`, `/agent run <name> <prompt>`.
  - Existing `/mode` can remain, but agent profile selection should be visible.
- Add desktop "Agent mode" selector using the same definitions.
- Add session/job projection for parallel agent runs:
  - agent id, session id, worktree/branch, status, latest proof, latest
    permission wait.
- Do not add broader autonomy until profile display and permissions are clear.

Candidate files:

- `src/agent/profiles.rs`
- `src/tools/agent_tool/mod.rs`
- `src/session_store/agent_store.rs`
- `src/tui/slash_handler/agents/agent_listing.rs`
- `src/tui/runtime_panels.rs`
- `apps/desktop/src/app/components/WorkbenchPanel.tsx`
- `apps/desktop/src/runtime/desktopApi.ts`

Acceptance:

- User can see available product profiles and their permissions.
- Starting an explore/review subagent shows progress and output contract.
- Parallel runs are visible as jobs, not just hidden transcript events.
- Permission mode is enforced per profile.
- Current implementation note: TUI `/agent switch` maps switchable product
  profiles onto `AgentMode`; `/agent run` delegates to the existing sub-agent
  tool. Desktop selector and richer job projection remain open.

Narrow gates:

```bash
cargo test -q agent_tool -- --test-threads=1
cargo test -q session_store
cargo test -q permissions
```

### Phase E: Desktop / IDE Context Bridge

Goal: carry editor-like context into the existing runtime without building a full
IDE extension first.

Code changes:

- Add `DesktopRunContext` variants for:
  - current file;
  - selected range;
  - active diff;
  - terminal cwd / command output reference.
- Add `@file` and `@file#Lx-Ly` parsing in TUI/desktop composer.
- Desktop:
  - add "Attach file" and "Attach selected range" commands using existing file
    picker first;
  - later, accept context from a small IDE helper command.
- Add a CLI helper:
  - `priority-agent context attach --file path --range 10:20`
  - writes a small local context handoff file or sends to desktop API if
    running.

Candidate files:

- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/app/components/Composer.tsx`
- `apps/desktop/src/app/components/ContextDetailDrawer.tsx`
- `apps/desktop/src-tauri/src/lib.rs`
- `src/main.rs`
- `src/engine/prompt_context.rs`
- `src/tools/file_tool/mod.rs`

Acceptance:

- A selected file/range appears as a labeled context card.
- The runtime prompt receives bounded file context with path/range provenance.
- Attaching context does not bypass file-read or permission semantics for later
  edits.
- Current implementation note: desktop `file` contexts now accept
  `line_start`, `line_end`, and `selection_text`, and the context drawer shows
  selected range metadata. TUI `@file#Lx-Ly`, CLI helper, and IDE extension are
  still open.

Narrow gates:

```bash
cargo test -q prompt_context
cargo test -q file_tool
```

### Phase F: Product Readiness Checks Before Soak Tests

Goal: keep broad testing deferred, but make every new product slice visible and
checkable.

Code changes:

- Extend the existing `/product-ready` report into the canonical product
  readiness DTO.
  - provider configured;
  - provider health recent;
  - selected model;
  - session store readable;
  - desktop settings path valid;
  - LSP disabled/enabled status;
  - export path writable;
  - permissions mode known;
  - runtime facade available.
- Add `/doctor product` as an alias if the doctor surface becomes the primary
  diagnostics entry.
- Add a desktop readiness panel using the same DTO instead of duplicating status
  logic in React/Tauri.
- Keep this as a diagnostic view, not an always-on prompt rule.

Candidate files:

- `src/tui/slash_handler/agents/doctor_formatting.rs`
- `src/tui/slash_handler/agents.rs`
- `apps/desktop/src-tauri/src/diagnostics.rs`
- `apps/desktop/src/app/components/DiagnosticsPanel.tsx`
- `docs/PROJECT_STATUS.md` only after implementation lands.

Acceptance:

- Product readiness gives a clear READY/BLOCKED/WARN status.
- It points to exact remediation actions.
- No secrets are printed.
- Current implementation note: desktop diagnostics now include product
  readiness checks from the same Rust DTO used by `/product-ready`; `/doctor
  product` is still open.

Narrow gates:

```bash
cargo test -q diagnostic
cargo test -q config
```

## 5. Suggested Implementation Order

1. Phase A provider catalog and `/connect` guidance.
   - This improves first-run and daily provider switching immediately.
2. Phase C export/share cleanup.
   - This removes misleading `/share` semantics and the placeholder bash export
     path.
3. Phase B LSP config and real `/lsp stop/restart`.
   - Start with control-plane correctness before post-edit diagnostic feedback.
4. Phase B post-edit optional diagnostics.
   - Only after LSP control plane is stable.
5. Phase D visible agent profiles.
   - Use existing profile machinery; avoid new orchestration until UI is clear.
6. Phase E desktop/IDE context bridge.
   - Start with desktop file/range attach, then CLI helper, then IDE extension.
7. Phase F product readiness.
   - Consolidate the new flows into one daily health view.

## 6. Non-Goals For This Phase

- No public cloud share link.
- No automatic LSP binary download.
- No weakening of checkpoint, stale-read, permissions, or validation closeout.
- No large soak/eval suite before product flows exist.
- No provider marketplace clone; keep provider support explicit and locally
  inspectable.

## 7. Documentation Updates After Implementation

When the code slices land, update:

- `docs/PROJECT_STATUS.md` for the current status anchor;
- `docs/README.md` if a new current doc should be discoverable;
- `AGENTS.md` only if runtime guidance changes;
- desktop/TUI onboarding docs if `/connect`, `/share local`, or LSP config
  become user-facing defaults.

## 8. Risk Notes

- Provider credentials are the highest UX/security risk. Do not store plaintext
  secrets in ordinary TOML without a deliberate decision.
- LSP can create false confidence if diagnostics are stale. Always label LSP
  diagnostics as optional evidence unless a validation command confirms the
  fix.
- Agent profiles can make the system look more autonomous than it is. The UI
  must show permission mode, worktree/session, and proof status.
- Session export can leak private code and prompts. Default to local redacted
  export; full export should be explicit.

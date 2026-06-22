# Phase A: Provider Setup And Onboarding — Detailed Implementation Plan

Date: 2026-06-10
Status: Completed
Parent: `docs/archive/NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`

## Goal

Make first-run provider setup feel product-ready. A new user should be able to
connect a provider from the product, paste a key, validate it, and start coding
without manually editing a shell profile or restarting the app.

Existing env-based providers must stay backward compatible. The implementation
must not weaken provider validation or report a provider as ready without real
runtime evidence.

## Current State From Code Audit

### Already Implemented

- `ProviderRegistry::from_env()` discovers the six built-in providers from
  provider-specific environment variables in deterministic order.
- `DEFAULT_PROVIDER_ENV_SPECS` in `src/services/api/provider.rs` is still the
  runtime registry source for provider ids, key env vars, base URLs, and default
  models.
- `provider_catalog.rs` adds product metadata: docs URLs, setup hints, default
  models, and supported model lists.
- `credentials.rs` can inspect credential status and render shell export hints,
  but it is read-only.
- `/connect <provider>` prints instructions; `/credentials`, `/key`, `/model`,
  and `/provider` expose provider-related TUI commands.
- TUI provider and model pickers already exist (`Ctrl+L` / `Ctrl+M`).
- TUI onboarding already exists through `src/onboarding/mod.rs`,
  `AppMode::Onboarding`, and `render_onboarding`, but it is reached only after
  provider bootstrap succeeds.
- Desktop already has backend commands for `provider_setup_info`,
  `provider_model_status`, and `set_provider_model`.
- Desktop state already persists `provider_name` and `model` in its own
  desktop settings JSON.
- Desktop Settings currently shows a static provider setup guide; the Settings
  drawer is not yet wired to the existing provider/model status and change
  handlers.

### Gaps And Corrections

1. **Credential persistence has no runtime contract.**
   Writing `PRIORITY_AGENT_API_KEY` is not sufficient today. The registry reads
   provider-specific variables such as `MINIMAX_API_KEY`, `DEEPSEEK_API_KEY`,
   `GLM_API_KEY`, and `OPENAI_API_KEY`. Startup also loads only the process
   environment and the current-directory `.env`, not `~/.priority-agent/.env`.

2. **No interactive credential write-back.**
   `/connect` still prints setup text. There is no safe write-back path, no
   immediate process-env refresh, and no "provider is available now" path.

3. **Provider metadata has multiple sources of truth.**
   Model lists are duplicated in the TUI picker, desktop state, and
   `provider_catalog.rs`. Provider ids/default models are also duplicated
   between `provider_catalog.rs` and `DEFAULT_PROVIDER_ENV_SPECS`. A real
   single-source pass must address or test both layers.

4. **TUI selected provider is not persisted in shared `AppConfig`.**
   TUI provider switching saves `api.model` and `api.base_url`, but there is no
   `api.provider_name`. Desktop has separate persistence, so this gap is
   TUI/shared-config specific.

5. **First-run provider onboarding is blocked before TUI starts.**
   The app has onboarding UI, but `init_app_or_exit()` exits on
   "No LLM provider configured" before `TuiApp` is created. The fix is a
   provider-less interactive bootstrap path for CLI/TUI only, not a broad change
   that makes API/eval/provider-health enter onboarding.

6. **Desktop Settings provider picker is an integration gap, not a backend gap.**
   `providerModelStatus()` and `setProviderModel()` already exist in the
   desktop API layer and App state. `SettingsDrawer` needs props, controls, and
   save-key wiring.

7. **Credential validation needs a light probe.**
   `--provider-health` runs a multi-step capability check. Onboarding should use
   a fast credential probe for save-time validation, while still reusing the
   existing health error categorization where possible.

8. **Existing onboarding module should be reused or replaced deliberately.**
   A new `src/tui/onboarding.rs` should not create a second onboarding system
   beside `src/onboarding/mod.rs` unless the old module is retired.

## Target Design

### Credential Persistence Contract

Add one explicit product credential store:

- path: `~/.priority-agent/.env`;
- format: standard dotenv key/value lines;
- saved key name: the selected provider's first catalog `key_env_vars` entry
  such as `MINIMAX_API_KEY`, never a generic `PRIORITY_AGENT_API_KEY`;
- optional saved default provider: `PRIORITY_AGENT_DEFAULT_PROVIDER=<provider>`;
- file permissions: owner-read/write only on Unix where supported;
- output: never print the secret value after save.

Startup must load this file before `ProviderRegistry::from_env()` is called.
The registry should remain backward compatible with shell env vars, current
directory `.env`, and `PRIORITY_AGENT_PROVIDER_<NAME>` custom provider entries.

### Provider Selection Contract

Add `api.provider_name: Option<String>` to shared `AppConfig` for TUI/shared
runtime selection. On startup:

1. Load environment and product credential env file.
2. Build the provider registry from the effective environment/config.
3. Prefer `api.provider_name` if that provider is configured.
4. Fall back to `PRIORITY_AGENT_DEFAULT_PROVIDER`.
5. Fall back to the existing deterministic provider order.

Desktop can continue using its desktop settings JSON for now, but the plan
should avoid drifting names and model defaults between desktop and shared config.

### Validation Contract

Saving a key has three possible outcomes:

- `verified`: key was saved, current process env was refreshed, registry can
  construct the provider, and the fast provider probe succeeded.
- `saved_unverified`: key was saved and registry can construct the provider,
  but validation failed for network/timeout/rate-limit reasons. UI must say it
  is saved but not verified.
- `rejected`: key was not saved, or was rolled back, because the provider id is
  unknown, the key is blank, the registry cannot construct the provider, or the
  provider returned an auth failure such as 401/403.

Do not display "Provider is now available" unless the result is `verified`.

## Implementation Plan

### Step 1: Credential Store And Startup Loading

**Files**:

- `src/services/api/credentials.rs`
- `src/services/api/provider_catalog.rs`
- `src/main.rs`
- `src/bootstrap.rs`

Tasks:

- Add helpers:
  - `credential_env_path() -> PathBuf`
  - `load_product_credential_env() -> Result<(), CredentialLoadError>`
  - `save_credential(provider_id: &str, key: &str) -> Result<CredentialSaveOutcome, ...>`
  - `set_env_for_session(var: &str, value: &str)`
- Save the provider-specific key env var from the catalog, for example
  `MINIMAX_API_KEY=<redacted>`.
- Also save or update `PRIORITY_AGENT_DEFAULT_PROVIDER=<provider_id>` so the
  newly connected provider wins when multiple keys are present.
- Load `~/.priority-agent/.env` before any `ProviderRegistry::from_env()` call
  on CLI/TUI/API/desktop startup.
- Preserve unknown dotenv lines and comments when updating the file.
- Do not write secrets to the repository, diagnostics, trace events, or command
  output.
- Add tests that write to a temporary credential env path rather than the real
  home directory.

Verification:

```bash
cargo test -q credentials -- --test-threads=1
cargo test -q provider -- --test-threads=1
cargo check -q
```

### Step 2: Consolidate Provider And Model Metadata

**Files**:

- `src/services/api/provider_catalog.rs`
- `src/services/api/provider.rs`
- `src/tui/app/palette.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`

Tasks:

- Use `provider_catalog::supported_models(provider_id)` in
  `TuiApp::model_choices()` instead of the hardcoded match.
- Use catalog-supported models in `desktop_model_options()` and remove
  `default_models_for_provider()` if no longer needed.
- Decide the runtime source of truth for built-in provider ids/defaults:
  - preferred: derive `DEFAULT_PROVIDER_ENV_SPECS`-equivalent runtime specs
    from catalog entries, or
  - acceptable first slice: add drift tests proving catalog entries and
    `DEFAULT_PROVIDER_ENV_SPECS` agree on id, env var, base URL, and default
    model.
- Remove hardcoded fallback model lists. For unknown/custom providers, fall back
  to the active model only.

Verification:

```bash
cargo test -q provider_catalog -- --test-threads=1
cargo test -q provider -- --test-threads=1
cargo test -q palette --lib -- --test-threads=1
```

### Step 3: Persist TUI Provider Selection In Shared Config

**Files**:

- `src/services/config.rs`
- `src/bootstrap.rs`
- `src/tui/app/palette.rs`

Tasks:

- Add `api.provider_name: Option<String>` to `ApiConfig`.
- Add a default for `api.provider_name` in `AppConfig::load()`.
- Update `bootstrap::init_provider()` to prefer configured
  `config.api.provider_name` when it points to an available provider.
- Save `api.provider_name` when TUI provider switching succeeds.
- Keep saving `api.model` and `api.base_url` for backward compatibility.
- Add tests for:
  - configured provider name wins when available;
  - unavailable provider name falls back without failing startup;
  - TUI provider switch persists provider name.

Verification:

```bash
cargo test -q config -- --test-threads=1
cargo test -q bootstrap -- --test-threads=1
cargo test -q palette --lib -- --test-threads=1
```

### Step 4: `/connect` Write-Back

**Files**:

- `src/tui/slash_handler/agents/auth_status.rs`
- `src/services/api/credentials.rs`
- `src/services/api/provider.rs`

Tasks:

- Parse `/connect <provider> <key>` without logging or echoing the key.
- If no key is provided, keep the existing instruction behavior.
- If a key is provided:
  - validate provider id against the catalog;
  - save the provider-specific env var;
  - set the env var in the current process;
  - set `PRIORITY_AGENT_DEFAULT_PROVIDER` in the current process;
  - rebuild or refresh the provider registry path used by the current runtime;
  - report `verified`, `saved_unverified`, or `rejected` accurately.
- Make `/credentials <provider>` show saved/configured status without revealing
  secret values.

Verification:

```bash
cargo test -q credentials -- --test-threads=1
cargo test -q auth_status -- --test-threads=1
```

### Step 5: Provider-Less Interactive Startup For CLI/TUI

**Files**:

- `src/main.rs`
- `src/bootstrap.rs`
- `src/onboarding/mod.rs`
- TUI rendering/input modules that currently handle `AppMode::Onboarding`

Tasks:

- Do not make `init_app()` silently succeed without a provider for all modes.
  API, eval, and `--provider-health` should still fail fast when no provider is
  configured.
- Add an explicit CLI/TUI-only startup path:
  - try normal provider bootstrap;
  - if no provider is configured, create a minimal provider-setup app state;
  - show the existing onboarding UI extended with provider picker/key input;
  - after a verified or saved-unverified credential write, retry provider
    bootstrap and enter the normal main loop when possible.
- Reuse or refactor `src/onboarding/mod.rs`; do not create a second unrelated
  onboarding module.
- Update old onboarding copy that says legacy mode works without a key if that
  is no longer true.
- Add a test-only way to simulate no configured provider, or use explicit
  env-var unsetting in tests. Do not document `PRIORITY_AGENT_NO_ENV=1` unless
  the implementation adds it.

Verification:

```bash
cargo test -q onboarding --lib -- --test-threads=1
cargo test -q bootstrap -- --test-threads=1
```

Manual smoke:

```bash
env -u MINIMAX_API_KEY -u KIMI_CODE_API_KEY -u DEEPSEEK_API_KEY \
  -u GLM_API_KEY -u ZAI_API_KEY -u ZHIPUAI_API_KEY -u BIGMODEL_API_KEY \
  -u MOONSHOT_API_KEY -u OPENAI_API_KEY \
  cargo run -- --tui
```

### Step 6: Desktop Settings Provider Controls

**Files**:

- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/app/components/SettingsDrawer.tsx`
- `apps/desktop/src/app/components/SettingsDrawer.css`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src-tauri/src/lib.rs`

Tasks:

- Pass `providerStatus` and `handleProviderModelChange()` into
  `SettingsDrawer`.
- Add provider and model controls to the Provider tab using the existing
  `providerModelStatus()` and `setProviderModel()` path.
- Disable model/provider selection for unconfigured providers unless the user is
  adding a key.
- Add a key input field and save button that calls a new Tauri command backed by
  the same credential-save helper used by `/connect`.
- Refresh provider/model status and desktop diagnostics after save.
- Keep "Open shell profile" as a secondary/manual option.

Verification:

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
```

### Step 7: Fast Credential Validation

**Files**:

- `src/services/api/credentials.rs`
- `src/diagnostics/provider_health.rs`
- `src/tui/slash_handler/agents/auth_status.rs`
- `src/onboarding/mod.rs`
- desktop Tauri credential-save command from Step 6

Tasks:

- Add a fast validation helper, for example:
  `validate_credential(provider_id: &str, key: &str, timeout: Duration)`.
- Build a temporary provider config from the selected catalog entry and the
  supplied key.
- Run a cheap plain-chat probe with low token budget and short timeout.
- Reuse `provider_health_error_category()` or equivalent categorization for
  auth/network/rate-limit messaging.
- Keep full `--provider-health` as a deeper post-setup diagnostic; do not make
  onboarding wait on the full tool-call health suite.
- Integrate validation into `/connect`, provider onboarding, and desktop key
  save.

Verification:

```bash
cargo test -q credentials -- --test-threads=1
cargo test -q provider_health --lib -- --test-threads=1
```

## Implementation Order

```text
Step 1 (credential store and startup loading)
  -> Step 2 (metadata/model-list consolidation)
  -> Step 3 (shared provider_name config)
  -> Step 4 (/connect write-back)
  -> Step 5 (provider-less CLI/TUI onboarding)
  -> Step 6 (desktop Settings controls)
  -> Step 7 (fast credential validation)
```

Steps 1-4 create the minimal usable loop for TUI users. Step 5 improves
first-run startup. Step 6 brings desktop parity. Step 7 can be introduced as a
strict verification gate once the save path is in place, but the public copy
must distinguish `verified` from `saved_unverified` from the start.

## Acceptance Criteria

- [ ] `~/.priority-agent/.env` is loaded before provider registry creation.
- [ ] Saved credentials use provider-specific env vars from the catalog.
- [ ] `/connect minimax <key>` can make MiniMax available in the current
      process without restart.
- [ ] Secret values are never printed, logged, or stored in repo files.
- [ ] Catalog and runtime provider specs either share one source or have drift
      tests.
- [ ] TUI model picker and desktop model picker no longer carry duplicate
      built-in model lists.
- [ ] `AppConfig` stores `api.provider_name`, and TUI provider selection
      survives restart.
- [ ] CLI/TUI no-provider startup reaches provider setup instead of exiting.
- [ ] API/eval/provider-health still fail fast when no provider is configured.
- [ ] Existing onboarding is reused or deliberately replaced, not duplicated.
- [ ] Desktop Settings uses the existing provider/model status path for
      interactive controls.
- [ ] Saved credentials are validated before showing a verified success.
- [ ] Existing env-based provider setup continues to work unchanged.

## Validation

```bash
cargo fmt --check
cargo check -q
cargo check --features experimental-api-server -q

# Targeted Rust
cargo test -q provider_catalog -- --test-threads=1
cargo test -q provider -- --test-threads=1
cargo test -q credentials -- --test-threads=1
cargo test -q config -- --test-threads=1
cargo test -q bootstrap -- --test-threads=1
cargo test -q onboarding --lib -- --test-threads=1
cargo test -q palette --lib -- --test-threads=1
cargo test -q provider_health --lib -- --test-threads=1

# Desktop
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build

# Full / release gate
cargo test -q
bash scripts/doc_health_check.sh
bash scripts/daily-baseline.sh
```

Run `cargo clippy --all-targets --all-features -- -D warnings` before merging
if the implementation changes provider construction, startup bootstrap, or API
DTO contracts.

## Non-Goals

- No public provider marketplace or broad 75+ provider support.
- No macOS Keychain integration in this phase; keep `from_keychain: false` as a
  future hook.
- No provider-specific request tuning or model routing beyond preserving the
  selected provider/model.
- No cloud credential storage or sync.
- No weakening of provider request/streaming boundaries, permissions,
  checkpoints, or validation gates to make a weaker provider appear healthy.

# Phase A: Provider Setup And Onboarding — Detailed Implementation Plan

Date: 2026-06-10
Status: Active
Parent: `docs/NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`

## Goal

Make first-run provider setup feel product-ready. A new user should be able to type
`/connect minimax`, paste their key, and start coding without manually editing a
shell profile or restarting the app. Existing env-based providers must stay
backward compatible.

## Current State (from code audit 2026-06-10)

### Already Implemented

- `ProviderRegistry::from_env()` — 6 built-in providers, env-based discovery
- `provider_catalog.rs` — centralized metadata with doc URLs, setup hints, model lists
- `credentials.rs` — read-only status inspection, shell export hints
- `/connect <provider>` — text instructions for each provider
- `/credentials` `/key` `/model` `/provider` — slash commands
- TUI provider/model pickers (Ctrl+L / Ctrl+M)
- Desktop `provider_setup_info`, `provider_model_status`, `set_provider_model` APIs
- Desktop Settings drawer with provider setup guide

### Gaps

1. **No interactive credential input.** `/connect` prints instructions; user must
   manually edit `~/.zshrc` and restart. No paste field, no env-file write-back,
   no Keychain integration.

2. **Model list duplication.** `palette.rs` has hardcoded per-provider model lists;
   `desktop_state.rs::default_models_for_provider()` has another copy;
   `provider_catalog.rs::builtin_catalog()` has a third. Only the catalog should
   be the source of truth.

3. **Provider name not persisted in `AppConfig`.** `api.model` and `api.base_url`
   are saved, but there's no `provider_name` field. TUI can't remember which
   provider was selected between sessions.

4. **No onboarding at first launch.** If no provider is configured, the app
   exits with "No LLM provider configured" instead of launching an interactive
   setup flow.

5. **Desktop Settings has no provider/model picker.** Only a static guide.
   No dropdown to select providers or models from Settings.

6. **No credential validation in onboarding flow.** Provider health check
   (`--provider-health`) exists but isn't integrated into setup.

## Implementation Plan

### Step 1: Consolidate Model Lists Into the Catalog

Remove the 2 duplicate model lists and make `provider_catalog::builtin_catalog()`
the single source of truth.

**Files**: `src/services/api/provider_catalog.rs`,
`src/tui/app/palette.rs`, `apps/desktop/src-tauri/src/desktop_state.rs`

Tasks:

- Verify `builtin_catalog()` has complete model lists for all 6 providers
- Change `palette.rs::model_choices()` to call
  `provider_catalog::supported_models(provider_id)` instead of hardcoded lists
- Change `desktop_state.rs::default_models_for_provider()` to use the catalog
- Keep backward compatibility: if a provider isn't in the catalog, fall back to
  the old hardcoded fallback list

Verification:

```bash
cargo test -q provider_catalog
cargo test -q palette --lib
```

### Step 2: Add `provider_name` to `AppConfig`

Add a `provider_name` field so the TUI can remember the last selected provider
across sessions.

**Files**: `src/services/config.rs`, `src/bootstrap.rs`

Tasks:

- Add `api.provider_name: Option<String>` to `ApiConfig`
- Add `set_default("api.provider_name", Option::<String>::None)` to `AppConfig::load()`
- Update `bootstrap.rs::init_provider()`: if `config.api.provider_name` is set,
  use it as the preferred provider (fall back to env-based selection)
- Save `provider_name` when switching providers in TUI (already done in
  `palette.rs::accept_provider_selection()`? — check and fix if not)

Verification:

```bash
cargo test -q config
```

### Step 3: Guided First-Run Onboarding

When no provider is configured at startup, instead of exiting with an error,
enter an interactive onboarding flow.

**Files**: `src/main.rs`, `src/bootstrap.rs`, new `src/tui/onboarding.rs`

Tasks:

- In `bootstrap.rs`, change `init_app()`: if `init_provider()` fails, do not exit;
  instead set a flag `onboarding_needed: bool`
- In `main.rs` TUI path: if `onboarding_needed`, launch an "Onboarding" TUI mode
  instead of the normal main loop
- New module `src/tui/onboarding.rs`:
  - Show a welcome screen: "Welcome to Priority Agent! Let's set up your provider."
  - Provider picker: list all 6 built-in providers with labels, descriptions,
    setup hints (one-liner about where to get the key)
  - Key input field: user pastes their API key
  - Validation: make a quick API call to verify the key works, show
    success/failure
  - Save: write `PRIORITY_AGENT_API_KEY=<key>` to `~/.priority-agent/.env` (and
    optionally offer to append to shell profile)
  - After save, initialize the provider and enter normal main loop
- Existing env-based providers stay working unchanged

Verification:

```bash
# Unset all keys and launch TUI — should see onboarding, not error
PRIORITY_AGENT_NO_ENV=1 cargo run -- --tui
cargo test -q onboarding --lib
```

### Step 4: `/connect` Write-Back

Make `/connect <provider>` actually save credentials instead of just printing
instructions.

**Files**: `src/tui/slash_handler/agents/auth_status.rs`,
`src/services/api/credentials.rs`

Tasks:

- Add `save_credential(provider_id: &str, key: &str)` to `src/services/api/credentials.rs`
  - Write to `~/.priority-agent/.env`: `PRIORITY_AGENT_API_KEY=<key>`
  - Use the first `key_env_vars` entry from the catalog as the variable name
- Add `set_env_for_session(key: &str, value: &str)` helper that calls
  `std::env::set_var()` so the current process picks it up immediately
- Update `/connect minimax <key>` to accept a key directly:
  - If key is provided: save, set env, show success
  - If key is not provided: show existing instructions + docs URL (current behavior)
- Add a confirmation: "Saved to ~/.priority-agent/.env. Provider is now available."

Verification:

```bash
cargo test -q credentials
```

### Step 5: Desktop Provider/Model Picker in Settings

Replace the static setup guide in the desktop Settings drawer with an
interactive provider selector.

**Files**: `apps/desktop/src/app/components/SettingsDrawer.tsx`,
`apps/desktop/src/app/components/SettingsDrawer.css`

Tasks:

- Add a provider/model dropdown to the Provider tab in Settings
- Use `providerModelStatus()` to get the list of configured/unconfigured providers
- Allow selecting/unselecting providers
- Show model picker for the selected provider
- "Add API key" section: text field + save button (calls a new Tauri command
  that writes to `~/.priority-agent/.env` and refreshes the runtime)
- Keep the existing "Open shell profile" button as a secondary option

Verification:

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
```

### Step 6: Validate Credentials On Save

When a user saves a new key, immediately validate it before confirming success.

**Files**: `src/services/api/credentials.rs`, `src/tui/onboarding.rs`

Tasks:

- Add `validate_credential(provider_id: &str, key: &str) -> Result<(), String>`
  - Create a temporary `ProviderConfig` with the given key
  - Make a quick chat request (e.g., "ping" with `max_tokens=1`)
  - Return `Ok(())` if successful, `Err(reason)` otherwise
  - Handle common errors: invalid key (401), network timeout, etc.
- Integrate into onboarding flow (Step 3) and `/connect` (Step 4)
- Show clear error messages: "Invalid API key — please check and try again" vs
  "Network error — saved locally, will retry on next use"

Verification:

```bash
cargo test -q credentials
cargo test -q provider_health --lib
```

## Implementation Order

```
Step 1 (consolidate model lists)
  → Step 2 (provider_name in AppConfig)
  → Step 3 (guided onboarding)
  → Step 4 (/connect write-back)
  → Step 5 (desktop picker)
  → Step 6 (credential validation)
```

Steps 1-4 and 6 are pure Rust. Step 5 is desktop frontend. Steps 1-2 are
small and safe refactors that make later steps cleaner.

## Acceptance Criteria

- [ ] `builtin_catalog()` is the single source of truth for all model lists
- [ ] TUI model picker reads from the catalog, no hardcoded duplicates
- [ ] `AppConfig` stores `api.provider_name` and TUI remembers it across sessions
- [ ] First launch with no provider shows interactive onboarding, not an error
- [ ] `/connect minimax <key>` saves the key and makes the provider immediately
      available without restart
- [ ] Desktop Settings has a provider/model picker dropdown
- [ ] Saved credentials are validated before showing success
- [ ] Existing env-based providers work unchanged
- [ ] All existing tests pass, new tests cover onboarding and validation paths

## Validation

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings

# Targeted
cargo test -q provider_catalog
cargo test -q credentials
cargo test -q config
cargo test -q provider_health --lib
cargo test -q palette --lib

# Desktop
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build

# Full
cargo test -q
bash scripts/daily-baseline.sh
```

## Non-Goals

- No public provider marketplace or 75+ provider support
- No macOS Keychain integration in this phase (keep `from_keychain: false` stub)
- No provider-specific request tuning or model routing
- No cloud credential storage or sync
- No changing the provider request/streaming boundary

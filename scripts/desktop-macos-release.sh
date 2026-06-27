#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="$ROOT_DIR/apps/desktop/test-artifacts"
EVIDENCE_DIR="$ROOT_DIR/docs/rc"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
EVIDENCE_FILE="$EVIDENCE_DIR/desktop-macos-release-$STAMP.md"

mkdir -p "$ARTIFACT_DIR" "$EVIDENCE_DIR"

commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
signing_identity="${PRIORITY_AGENT_MACOS_SIGNING_IDENTITY:-unsigned-local}"
notarization_profile="${PRIORITY_AGENT_MACOS_NOTARY_PROFILE:-}"
supply_chain_status="skipped_set_PRIORITY_AGENT_DESKTOP_RELEASE_SUPPLY_CHAIN=1"
native_rc_smoke_status="skipped_set_PRIORITY_AGENT_DESKTOP_RELEASE_NATIVE_SMOKE=1"

run_step() {
  local name="$1"
  shift
  echo "==> $name"
  "$@"
}

run_step "desktop frontend build" corepack pnpm --dir "$ROOT_DIR/apps/desktop" build
run_step "desktop UI smoke" corepack pnpm --dir "$ROOT_DIR/apps/desktop" test:ui-smoke
run_step "desktop release security check" bash "$ROOT_DIR/scripts/check_desktop_release_security.sh"
if [[ "${PRIORITY_AGENT_DESKTOP_RELEASE_SUPPLY_CHAIN:-0}" == "1" ]]; then
  run_step "supply chain audit" bash "$ROOT_DIR/scripts/security_dependency_audit.sh"
  supply_chain_status="passed"
fi
run_step "Tauri backend tests" cargo test --manifest-path "$ROOT_DIR/apps/desktop/src-tauri/Cargo.toml" -- --test-threads=1
run_step "Tauri build" corepack pnpm --dir "$ROOT_DIR/apps/desktop" tauri build
if [[ "${PRIORITY_AGENT_DESKTOP_RELEASE_NATIVE_SMOKE:-0}" == "1" ]]; then
  run_step "desktop native RC failure smoke" bash "$ROOT_DIR/scripts/desktop-native-smoke.sh" --skip-build --rc-failure-check
  native_rc_smoke_status="passed"
fi

notarization_status="not_requested"
if [[ "$signing_identity" != "unsigned-local" && -n "$notarization_profile" ]]; then
  dmg_path="$(find "$ROOT_DIR/apps/desktop/src-tauri/target/release/bundle/dmg" -maxdepth 1 -name '*.dmg' | head -n 1 || true)"
  if [[ -z "$dmg_path" ]]; then
    echo "No DMG found for notarization" >&2
    exit 1
  fi
  echo "==> submit notarization"
  xcrun notarytool submit "$dmg_path" --keychain-profile "$notarization_profile" --wait
  xcrun stapler staple "$dmg_path"
  spctl --assess --type open --context context:primary-signature -v "$dmg_path"
  notarization_status="submitted_and_stapled"
fi

cat >"$EVIDENCE_FILE" <<EOF
# Desktop macOS Release Evidence - $STAMP

- commit: $commit
- signing_identity: $signing_identity
- notarization_profile: ${notarization_profile:-not_configured}
- notarization_status: $notarization_status
- frontend_build: passed
- ui_smoke: passed
- release_security_check: passed
- supply_chain_audit: $supply_chain_status
- tauri_backend_tests: passed
- tauri_build: passed
- native_rc_failure_smoke: $native_rc_smoke_status
- known_limitations:
  - macOS-first desktop package.
  - Windows/Linux desktop packages not validated by this evidence.
  - Controlled validation is not a sandbox for untrusted repositories.
EOF

echo "desktop macOS release evidence: $EVIDENCE_FILE"

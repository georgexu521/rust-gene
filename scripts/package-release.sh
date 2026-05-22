#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

VERSION=""
FEATURES=""
TARGET_TRIPLE="$(rustc -vV | awk '/host:/ {print $2}')"
DIST_DIR="$ROOT_DIR/target/dist"
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/package-release.sh [options]

Options:
  --version <version>    Version label for the archive (default: Cargo.toml package version)
  --features <features>  Cargo features for release build
  --target <triple>      Target triple label/build target (default: rustc host)
  --dist-dir <path>      Output directory (default: target/dist)
  --dry-run              Print package plan without building or writing artifacts
  -h, --help             Show this help

Outputs:
  priority-agent-<version>-<target>.tar.gz
  priority-agent-<version>-<target>.tar.gz.sha256
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --features)
      FEATURES="${2:-}"
      shift 2
      ;;
    --target)
      TARGET_TRIPLE="${2:-}"
      shift 2
      ;;
    --dist-dir)
      DIST_DIR="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
fi

if [[ -z "$VERSION" || -z "$TARGET_TRIPLE" || -z "$DIST_DIR" ]]; then
  echo "version, target, and dist-dir must be non-empty" >&2
  exit 2
fi

ARCHIVE_BASE="priority-agent-${VERSION}-${TARGET_TRIPLE}"
ARCHIVE_PATH="$DIST_DIR/${ARCHIVE_BASE}.tar.gz"
BUILD_ARGS=(build --release)
if [[ -n "$FEATURES" ]]; then
  BUILD_ARGS+=(--features "$FEATURES")
fi
if [[ -n "$TARGET_TRIPLE" ]]; then
  BUILD_ARGS+=(--target "$TARGET_TRIPLE")
fi

echo "Release package plan:"
echo "  version:  $VERSION"
echo "  target:   $TARGET_TRIPLE"
echo "  features: ${FEATURES:-<none>}"
echo "  dist:     $DIST_DIR"
echo "  archive:  $ARCHIVE_PATH"

if [[ "$DRY_RUN" == "1" ]]; then
  exit 0
fi

cargo "${BUILD_ARGS[@]}"

BIN_PATH="$ROOT_DIR/target/$TARGET_TRIPLE/release/priority-agent"
if [[ ! -x "$BIN_PATH" ]]; then
  echo "release binary not found or not executable: $BIN_PATH" >&2
  exit 1
fi

STAGE_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$STAGE_DIR"
}
trap cleanup EXIT

mkdir -p "$STAGE_DIR/$ARCHIVE_BASE" "$DIST_DIR"
cp "$BIN_PATH" "$STAGE_DIR/$ARCHIVE_BASE/priority-agent"
cp README.md "$STAGE_DIR/$ARCHIVE_BASE/README.md" 2>/dev/null || true
cp scripts/install.sh "$STAGE_DIR/$ARCHIVE_BASE/install.sh"

tar -C "$STAGE_DIR" -czf "$ARCHIVE_PATH" "$ARCHIVE_BASE"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$DIST_DIR" && sha256sum "$(basename "$ARCHIVE_PATH")" >"$(basename "$ARCHIVE_PATH").sha256")
else
  (cd "$DIST_DIR" && shasum -a 256 "$(basename "$ARCHIVE_PATH")" >"$(basename "$ARCHIVE_PATH").sha256")
fi

echo "Wrote:"
echo "  $ARCHIVE_PATH"
echo "  $ARCHIVE_PATH.sha256"

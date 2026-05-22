#!/usr/bin/env bash
# Priority Agent 一键安装脚本
# Usage: ./scripts/install.sh [--release] [--prefix /usr/local]

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BUILD_TYPE="debug"
FEATURES="${FEATURES:-}"
SKIP_BUILD=0
SKIP_VERIFY=0
DRY_RUN=0
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
BIN_DIR=""
CONFIG_DIR=""
VERSION=""

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [options]

Options:
  --version          Print package version and exit
  --release          Build in release mode (default: debug)
  --features F       Comma-separated cargo features (default: none)
  --no-cli           Deprecated compatibility flag; ignored
  --skip-build       Skip cargo build and install existing binary from target/
  --skip-verify      Skip final binary smoke-check
  --dry-run          Print install plan and dependency diagnostics, then exit
  --prefix PATH      Install prefix directory (default: ~/.local)
  --system           Install to /usr/local (requires sudo)
  -h, --help         Show this help

Examples:
  scripts/install.sh --version
  scripts/install.sh --dry-run --release
  scripts/install.sh --release
  scripts/install.sh --release --features experimental-api-server
  scripts/install.sh --release --skip-build
  scripts/install.sh --release --system
EOF
}

detect_version() {
  awk -F' *= *' '/^version *=/ { gsub(/"/, "", $2); print $2; exit }' Cargo.toml
}

print_toolchain_diagnostics() {
  if command -v cargo &>/dev/null; then
    echo "Cargo:       $(cargo --version)"
  else
    echo "Cargo:       missing"
  fi
  if command -v rustc &>/dev/null; then
    echo "Rustc:       $(rustc --version)"
  else
    echo "Rustc:       missing"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) echo "$(detect_version)"; exit 0 ;;
    --release) BUILD_TYPE="release"; shift ;;
    --features) FEATURES="${2:-}"; shift 2 ;;
    --no-cli) FEATURES=""; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-verify) SKIP_VERIFY=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --prefix) INSTALL_PREFIX="${2:-}"; shift 2 ;;
    --system) INSTALL_PREFIX="/usr/local"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

BIN_DIR="$INSTALL_PREFIX/bin"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/priority-agent"
VERSION="$(detect_version)"

echo "=== Priority Agent Installer ==="
echo ""
echo "Version:      $VERSION"
echo "Build type:   $BUILD_TYPE"
echo "Features:     ${FEATURES:-(none)}"
echo "Skip build:   $SKIP_BUILD"
echo "Skip verify:  $SKIP_VERIFY"
echo "Dry run:      $DRY_RUN"
echo "Install prefix: $INSTALL_PREFIX"
echo "Binary dir:   $BIN_DIR"
echo "Config dir:   $CONFIG_DIR"
echo ""
print_toolchain_diagnostics
echo ""

if [[ -z "$INSTALL_PREFIX" || "$INSTALL_PREFIX" == "/" ]]; then
  echo "Error: unsafe install prefix: '${INSTALL_PREFIX:-<empty>}'" >&2
  exit 1
fi

# Check Rust toolchain
if ! command -v cargo &>/dev/null; then
  echo "Error: Rust/Cargo not found. Please install Rust first:"
  echo "  https://rustup.rs/"
  exit 1
fi

if ! command -v rustc &>/dev/null; then
  echo "Error: rustc not found. Please install Rust first:"
  echo "  https://rustup.rs/"
  exit 1
fi

if [[ "$BUILD_TYPE" == "release" ]]; then
  SRC_BIN="target/release/priority-agent"
else
  SRC_BIN="target/debug/priority-agent"
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Dry run plan:"
  echo "  source binary: $SRC_BIN"
  echo "  install binary: $BIN_DIR/priority-agent"
  echo "  shortcut: $BIN_DIR/pa"
  echo "  config dir: $CONFIG_DIR"
  echo "No files were changed."
  exit 0
fi

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  echo "[1/4] Building priority-agent..."
  BUILD_START_TS="$(date +%s)"
  CARGO_ARGS=("--bin" "priority-agent")
  if [[ "$BUILD_TYPE" == "release" ]]; then
    CARGO_ARGS+=("--release")
  fi
  if [[ -n "$FEATURES" ]]; then
    CARGO_ARGS+=("--features" "$FEATURES")
    echo "       Features: $FEATURES"
  fi
  cargo build --quiet "${CARGO_ARGS[@]}"
  BUILD_END_TS="$(date +%s)"
  echo "       Build done in $((BUILD_END_TS - BUILD_START_TS))s"
else
  echo "[1/4] Skipping build, using existing binary: $SRC_BIN"
fi

if [[ ! -x "$SRC_BIN" ]]; then
  if [[ "$SKIP_BUILD" -eq 1 ]]; then
    echo "Error: --skip-build was set but binary not found at $SRC_BIN"
    echo "       Run without --skip-build first."
  else
    echo "Error: Build failed - binary not found at $SRC_BIN"
  fi
  exit 1
fi

echo "[2/4] Installing binary to $BIN_DIR..."
mkdir -p "$BIN_DIR"
BACKUP_BIN=""
if [[ -f "$BIN_DIR/priority-agent" ]]; then
  BACKUP_BIN="$(mktemp "${TMPDIR:-/tmp}/priority-agent-install-backup.XXXXXX")"
  cp "$BIN_DIR/priority-agent" "$BACKUP_BIN"
fi

if ! /usr/bin/install -m 0755 "$SRC_BIN" "$BIN_DIR/priority-agent"; then
  if [[ -n "$BACKUP_BIN" && -f "$BACKUP_BIN" ]]; then
    echo "       Install failed; restoring previous binary."
    /usr/bin/install -m 0755 "$BACKUP_BIN" "$BIN_DIR/priority-agent" || true
  fi
  exit 1
fi

# 创建 pa symlink（快捷命令，默认启动 Priority Agent）
if ! ln -sf "$BIN_DIR/priority-agent" "$BIN_DIR/pa"; then
  if [[ -n "$BACKUP_BIN" && -f "$BACKUP_BIN" ]]; then
    echo "       Shortcut creation failed; restoring previous binary."
    /usr/bin/install -m 0755 "$BACKUP_BIN" "$BIN_DIR/priority-agent" || true
  fi
  exit 1
fi
if [[ -n "$BACKUP_BIN" ]]; then
  rm -f "$BACKUP_BIN"
fi
echo "       Created shortcut: $BIN_DIR/pa -> priority-agent"

echo "[3/4] Creating config directory $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"

# Create default config if not exists
if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
  cat > "$CONFIG_DIR/config.toml" <<'EOF'
# Priority Agent Configuration
# See AGENTS.md for documentation

[ui]
theme = "dark"

[features]
# plugin_trust_mode = "warn"
EOF
  echo "       Created default config: $CONFIG_DIR/config.toml"
fi

# Create .env.example if not exists
if [[ ! -f "$CONFIG_DIR/.env" ]]; then
  cat > "$CONFIG_DIR/.env" <<'EOF'
# LLM API Keys (at least one is required)
# MINIMAX_API_KEY=""
# OPENAI_API_KEY=""
# MOONSHOT_API_KEY=""
EOF
  echo "       Created env template: $CONFIG_DIR/.env"
fi

echo "[4/4] Verifying installation..."
if [[ "$SKIP_VERIFY" -eq 1 ]]; then
  echo "       Skipped."
else
  if "$BIN_DIR/priority-agent" --help &>/dev/null; then
    echo "       OK: binary works"
  else
    echo "       Warning: binary test failed"
  fi
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Version:    $VERSION"
echo "Binary:     $BIN_DIR/priority-agent"
echo "Shortcut:   $BIN_DIR/pa"
echo "Config:     $CONFIG_DIR/"
echo ""
# Warn if prefix bin is not in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo "Note: $BIN_DIR is not in your PATH."
  echo "      Add this to your shell profile:"
  echo "        export PATH=\"$BIN_DIR:\$PATH\""
  echo ""
fi
echo "Next steps:"
echo "  1. Set your LLM API key:"
echo "     export MOONSHOT_API_KEY='your-key-here'"
echo "  2. Or edit: $CONFIG_DIR/.env"
echo "  3. Run: pa                  # start Priority Agent"
echo "     Run: priority-agent      # same command, full name"
echo ""

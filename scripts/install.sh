#!/usr/bin/env bash
set -euo pipefail

REPO_OWNER="JJacksonchen"
REPO_NAME="antigravity-island"
REPO_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}.git"
BUNDLE_ID="com.jjacksonchen.antigravity-island"
APP_NAME="Antigravity Island.app"

AUTOSTART=0
NO_LAUNCH=0
FROM_SOURCE=0
PREFIX="${HOME}/.local/bin"
APP_DIR="/Applications"

usage() {
  cat <<'EOF'
Antigravity Island One-Click Installer

Usage:
  install.sh [OPTIONS]

Options:
  --autostart       Install macOS LaunchAgent to start automatically at login
  --no-launch       Do not open the app after installing
  --from-source     Force building from source with cargo instead of prebuilt binary
  --prefix <dir>    CLI binary install directory (default: ~/.local/bin)
  --app-dir <dir>   macOS .app install directory (default: /Applications, fallback ~/Applications)
  -h, --help        Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --autostart) AUTOSTART=1; shift ;;
    --no-launch) NO_LAUNCH=1; shift ;;
    --from-source) FROM_SOURCE=1; shift ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    --app-dir) APP_DIR="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

info() { printf '\033[1;36m==>\033[0m %s\n' "$1"; }
ok()   { printf '\033[1;32m✓\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$1"; }
err()  { printf '\033[1;31m✗\033[0m %s\n' "$1" >&2; }

OS="$(uname -s)"
ARCH="$(uname -m)"

TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

BIN_SRC=""

try_prebuilt_macos() {
  local asset_arch=""
  case "$ARCH" in
    arm64|aarch64) asset_arch="arm64" ;;
    x86_64) asset_arch="x86_64" ;;
    *) return 1 ;;
  esac

  local url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/antigravity-island-macos-${asset_arch}.tar.gz"
  info "Downloading prebuilt binary (${asset_arch}) from GitHub Releases..."
  if curl -fsSL "$url" -o "$TMP_DIR/release.tar.gz"; then
    tar -xzf "$TMP_DIR/release.tar.gz" -C "$TMP_DIR"
    if [[ -x "$TMP_DIR/antigravity-island" ]]; then
      BIN_SRC="$TMP_DIR/antigravity-island"
      return 0
    fi
  fi
  return 1
}

build_from_source() {
  if ! command -v cargo >/dev/null 2>&1; then
    err "Rust toolchain (cargo) is required to build from source."
    echo "Install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
  fi

  local src_dir="$TMP_DIR/src"
  if [[ -n "${ANTIGRAVITY_ISLAND_LOCAL_REPO:-}" && -d "${ANTIGRAVITY_ISLAND_LOCAL_REPO}" ]]; then
    info "Using local repository: ${ANTIGRAVITY_ISLAND_LOCAL_REPO}"
    mkdir -p "$src_dir"
    cp -R "${ANTIGRAVITY_ISLAND_LOCAL_REPO}/Cargo.toml" "${ANTIGRAVITY_ISLAND_LOCAL_REPO}/Cargo.lock" \
      "${ANTIGRAVITY_ISLAND_LOCAL_REPO}/src" "$src_dir/"
    if [[ -d "${ANTIGRAVITY_ISLAND_LOCAL_REPO}/tests" ]]; then
      cp -R "${ANTIGRAVITY_ISLAND_LOCAL_REPO}/tests" "$src_dir/"
    fi
  else
    if ! command -v git >/dev/null 2>&1; then
      err "git is required to clone ${REPO_URL}"
      exit 1
    fi
    info "Cloning ${REPO_URL}..."
    git clone --depth 1 "$REPO_URL" "$src_dir"
  fi

  info "Building optimized release binary with cargo..."
  cargo build --release --manifest-path "$src_dir/Cargo.toml"
  BIN_SRC="$src_dir/target/release/antigravity-island"
}

if [[ "$OS" == "Darwin" && "$FROM_SOURCE" -eq 0 && -z "${ANTIGRAVITY_ISLAND_LOCAL_REPO:-}" ]]; then
  if ! try_prebuilt_macos; then
    warn "Prebuilt release not found or download failed; falling back to source build."
    build_from_source
  fi
else
  build_from_source
fi

if [[ ! -f "$BIN_SRC" ]]; then
  err "Compiled binary not found at $BIN_SRC"
  exit 1
fi

mkdir -p "$PREFIX"
cp "$BIN_SRC" "$PREFIX/antigravity-island"
chmod +x "$PREFIX/antigravity-island"
ok "Installed CLI binary: $PREFIX/antigravity-island"

APP_BUNDLE_PATH=""
if [[ "$OS" == "Darwin" ]]; then
  mkdir -p "$APP_DIR" >/dev/null 2>&1 || true
  if [[ ! -w "$APP_DIR" ]]; then
    APP_DIR="${HOME}/Applications"
    mkdir -p "$APP_DIR"
  fi
  APP_BUNDLE_PATH="${APP_DIR}/${APP_NAME}"
  info "Packaging macOS app bundle at ${APP_BUNDLE_PATH}..."
  rm -rf "$APP_BUNDLE_PATH"
  mkdir -p "$APP_BUNDLE_PATH/Contents/MacOS" "$APP_BUNDLE_PATH/Contents/Resources"
  cp "$BIN_SRC" "$APP_BUNDLE_PATH/Contents/MacOS/antigravity-island"
  chmod +x "$APP_BUNDLE_PATH/Contents/MacOS/antigravity-island"

  cat > "$APP_BUNDLE_PATH/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>antigravity-island</string>
  <key>CFBundleIdentifier</key>
  <string>com.jjacksonchen.antigravity-island</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Antigravity Island</string>
  <key>CFBundleDisplayName</key>
  <string>Antigravity Island</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.2.1</string>
  <key>CFBundleVersion</key>
  <string>3</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
  ok "Installed macOS app: ${APP_BUNDLE_PATH}"
fi

"$PREFIX/antigravity-island" --list-skins >/dev/null || true
ok "Initialized skins directory"

if [[ "$OS" == "Darwin" && "$AUTOSTART" -eq 1 ]]; then
  AGENTS_DIR="${HOME}/Library/LaunchAgents"
  PLIST_PATH="${AGENTS_DIR}/${BUNDLE_ID}.plist"
  mkdir -p "$AGENTS_DIR"
  cat > "$PLIST_PATH" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>${BUNDLE_ID}</string>
  <key>ProgramArguments</key>
  <array>
    <string>${APP_BUNDLE_PATH}/Contents/MacOS/antigravity-island</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
PLIST
  launchctl unload "$PLIST_PATH" >/dev/null 2>&1 || true
  launchctl load -w "$PLIST_PATH" >/dev/null 2>&1 || true
  ok "Configured login LaunchAgent: ${PLIST_PATH}"
fi

echo ""
echo "Antigravity Island installed successfully!"
echo "  Probe live quota : ${PREFIX}/antigravity-island --probe"
echo "  Preview demo UI  : ${PREFIX}/antigravity-island --demo"
if [[ -n "$APP_BUNDLE_PATH" ]]; then
  echo "  Launch macOS app : open \"${APP_BUNDLE_PATH}\""
fi

if [[ "$OS" == "Darwin" && "$NO_LAUNCH" -eq 0 && -n "$APP_BUNDLE_PATH" ]]; then
  if [[ "$AUTOSTART" -eq 1 ]]; then
    ok "Antigravity Island started via LaunchAgent"
  else
    pkill -x antigravity-island >/dev/null 2>&1 || true
    sleep 0.3
    open "$APP_BUNDLE_PATH"
    ok "Launched Antigravity Island"
  fi
fi

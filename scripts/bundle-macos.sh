#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="Antigravity Island.app"
BUNDLE_DIR="$ROOT_DIR/$APP_NAME"
BIN_PATH="$ROOT_DIR/target/release/antigravity-island"

echo "Building release binary..."
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml"

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS" "$BUNDLE_DIR/Contents/Resources"

cp "$BIN_PATH" "$BUNDLE_DIR/Contents/MacOS/antigravity-island"
chmod +x "$BUNDLE_DIR/Contents/MacOS/antigravity-island"

cat > "$BUNDLE_DIR/Contents/Info.plist" <<'PLIST'
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
  <string>0.2.3</string>
  <key>CFBundleVersion</key>
  <string>5</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

echo "Bundled successfully at: $BUNDLE_DIR"
echo "Run: open \"$BUNDLE_DIR\""

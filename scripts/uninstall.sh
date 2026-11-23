#!/usr/bin/env bash
set -euo pipefail

BUNDLE_ID="com.jjacksonchen.antigravity-island"
PURGE_CONFIG=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --purge-config) PURGE_CONFIG=1; shift ;;
    -h|--help)
      echo "Usage: uninstall.sh [--purge-config]"
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

pkill -x antigravity-island >/dev/null 2>&1 || true

PLIST_PATH="${HOME}/Library/LaunchAgents/${BUNDLE_ID}.plist"
if [[ -f "$PLIST_PATH" ]]; then
  launchctl unload "$PLIST_PATH" >/dev/null 2>&1 || true
  rm -f "$PLIST_PATH"
  echo "Removed LaunchAgent: $PLIST_PATH"
fi

for app_path in \
  "/Applications/Antigravity Island.app" \
  "${HOME}/Applications/Antigravity Island.app"; do
  if [[ -d "$app_path" ]]; then
    rm -rf "$app_path"
    echo "Removed app bundle: $app_path"
  fi
done

for bin_path in \
  "${HOME}/.local/bin/antigravity-island" \
  "/usr/local/bin/antigravity-island"; do
  if [[ -f "$bin_path" ]]; then
    rm -f "$bin_path"
    echo "Removed binary: $bin_path"
  fi
done

if [[ "$PURGE_CONFIG" -eq 1 && -d "${HOME}/.config/antigravity-island" ]]; then
  rm -rf "${HOME}/.config/antigravity-island"
  echo "Removed config directory: ${HOME}/.config/antigravity-island"
fi

echo "Antigravity Island uninstalled."

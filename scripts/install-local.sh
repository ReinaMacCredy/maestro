#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${MAESTRO_INSTALL_DIR:-$HOME/.local/bin}"
SOURCE_BIN="${1:-./dist/maestro}"
TARGET_BIN="$INSTALL_DIR/maestro"

if [ ! -f "$SOURCE_BIN" ]; then
  echo "[!!] Built binary not found at $SOURCE_BIN" >&2
  echo "     Run: bun run build" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
TMP_BIN="$(mktemp "$INSTALL_DIR/.maestro.tmp.XXXXXX")"
cp "$SOURCE_BIN" "$TMP_BIN"
chmod +x "$TMP_BIN"
mv "$TMP_BIN" "$TARGET_BIN"

INSTALLED_PATH="$(command -v maestro || true)"
INSTALLED_VERSION="$("$TARGET_BIN" --version)"

echo "[ok] Installed maestro $INSTALLED_VERSION to $TARGET_BIN"
if [ -n "$INSTALLED_PATH" ]; then
  echo "[ok] PATH maestro resolves to $INSTALLED_PATH"
else
  echo "[--] maestro is not currently on PATH"
fi

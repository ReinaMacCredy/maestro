#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${MAESTRO_INSTALL_DIR:-$HOME/.local/bin}"
RELEASE_REPO="${MAESTRO_RELEASE_REPO:-ReinaMacCredy/maestro}"
REQUESTED_VERSION="${MAESTRO_VERSION:-latest}"
TARGET_BIN="$INSTALL_DIR/maestro"

info()  { echo "[ok] $*"; }
warn()  { echo "[!] $*"; }
fail()  { echo "[!] $*" >&2; exit 1; }

main() {
  echo "maestro release installer"
  echo ""

  command -v curl >/dev/null 2>&1 || fail "curl is required."

  local asset url
  asset="$(resolve_asset_name)"
  url="$(build_download_url "$asset")"

  mkdir -p "$INSTALL_DIR"
  TMP_BIN="$(mktemp "$INSTALL_DIR/.maestro.tmp.XXXXXX")"
  trap 'rm -f "$TMP_BIN"' EXIT

  echo "Installing asset: $asset"
  echo "Download URL: $url"
  echo ""

  curl -fsSL "$url" -o "$TMP_BIN"
  chmod +x "$TMP_BIN"
  mv "$TMP_BIN" "$TARGET_BIN"

  if "$TARGET_BIN" --version >/dev/null 2>&1; then
    info "Installed maestro $("$TARGET_BIN" --version) to $TARGET_BIN"
  else
    fail "Installation verification failed"
  fi

  local resolved_path
  resolved_path="$(command -v maestro || true)"
  if [ -n "$resolved_path" ]; then
    info "PATH maestro resolves to $resolved_path"
  fi

  if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo ""
    warn "$INSTALL_DIR is not in your PATH"
    echo "    Add: export PATH=\"$INSTALL_DIR:\$PATH\""
  fi

  echo ""
  info "Running maestro install..."
  "$TARGET_BIN" install
}

build_download_url() {
  local asset="$1"
  local base_url="https://github.com/$RELEASE_REPO/releases"
  if [ "$REQUESTED_VERSION" = "latest" ]; then
    printf "%s/latest/download/%s" "$base_url" "$asset"
    return
  fi

  local tag="$REQUESTED_VERSION"
  case "$tag" in
    v*) ;;
    *) tag="v$tag" ;;
  esac
  printf "%s/download/%s/%s" "$base_url" "$tag" "$asset"
}

resolve_asset_name() {
  local os arch
  case "$(uname -s)" in
    Darwin) os="darwin" ;;
    Linux) os="linux" ;;
    *) fail "Unsupported platform: $(uname -s). Release installs support macOS and Linux." ;;
  esac

  case "$(uname -m)" in
    arm64|aarch64) arch="arm64" ;;
    x86_64|amd64) arch="x64" ;;
    *) fail "Unsupported architecture: $(uname -m). Release installs support x64 and arm64." ;;
  esac

  printf "maestro-%s-%s" "$os" "$arch"
}

main "$@"

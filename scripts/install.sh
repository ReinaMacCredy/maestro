#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${MAESTRO_INSTALL_DIR:-$HOME/.local/bin}"
RELEASE_REPO="ReinaMacCredy/maestro"
REQUESTED_VERSION="${MAESTRO_VERSION:-latest}"
TARGET_BIN="$INSTALL_DIR/maestro"

info()  { echo "[ok] $*"; }
warn()  { echo "[!] $*"; }
fail()  { echo "[!] $*" >&2; exit 1; }

main() {
  echo "maestro release installer"
  echo ""

  command -v curl >/dev/null 2>&1 || fail "curl is required."

  local asset url checksum_url
  asset="$(resolve_asset_name)"
  url="$(build_download_url "$asset")"
  checksum_url="${url}.sha256"

  mkdir -p "$INSTALL_DIR"
  TMP_BIN="$(mktemp "$INSTALL_DIR/.maestro.tmp.XXXXXX")"
  TMP_CHECKSUM="$(mktemp "$INSTALL_DIR/.maestro.sha256.XXXXXX")"
  trap 'rm -f "$TMP_BIN" "$TMP_CHECKSUM"' EXIT

  echo "Installing asset: $asset"
  echo "Download URL: $url"
  echo ""

  curl -fsSL "$url" -o "$TMP_BIN"
  curl -fsSL "$checksum_url" -o "$TMP_CHECKSUM"
  verify_checksum "$TMP_BIN" "$TMP_CHECKSUM" "$asset"
  chmod +x "$TMP_BIN"
  remove_shadowing_maestros
  mv "$TMP_BIN" "$TARGET_BIN"

  if "$TARGET_BIN" version >/dev/null 2>&1; then
    info "Installed $("$TARGET_BIN" version | head -n1) to $TARGET_BIN"
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
}

# Q3 PATH hygiene: a maestro in a PATH dir *before* the install dir would
# shadow the freshly installed binary. Remove only those so the new install
# wins; warn, don't fail, when a path needs elevated permissions. A maestro at
# or after the install dir already loses to the new binary and is left alone --
# this keeps the blast radius to binaries that actually break the install (an
# unrelated tool named `maestro`, or a dev's `~/.cargo/bin/maestro`, sitting
# later on PATH is untouched). The binary is removed outright, not backed up:
# it is freely re-downloadable, and the "never delete" guarantee covers user
# data (MIGRATE.md), not the binary. If the install dir is not on PATH, the new
# binary can't win regardless (main() warns about that), so remove nothing.
remove_shadowing_maestros() {
  local dir candidate IFS=:
  # First confirm the install dir is on PATH; if it is not, the new binary can't
  # win regardless, so removing other maestros would only orphan the user.
  local on_path=0
  for dir in $PATH; do
    [ -n "$dir" ] || continue
    if [ "$dir" -ef "$INSTALL_DIR" ]; then on_path=1; break; fi
  done
  [ "$on_path" = 1 ] || return 0

  for dir in $PATH; do
    [ -n "$dir" ] || continue
    # Stop at the install dir: only earlier PATH entries can shadow it.
    if [ "$dir" -ef "$INSTALL_DIR" ]; then return 0; fi
    candidate="$dir/maestro"
    [ -f "$candidate" ] || continue
    if [ "$candidate" -ef "$TARGET_BIN" ]; then continue; fi
    if rm -f "$candidate" 2>/dev/null; then
      warn "Removed shadowing maestro: $candidate"
    else
      warn "Another maestro on PATH at $candidate blocks this install and could not be removed."
      echo "    Remove it manually: sudo rm \"$candidate\""
    fi
  done
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
    x86_64|amd64) arch="amd64" ;;
    *) fail "Unsupported architecture: $(uname -m). Release installs support amd64 and arm64." ;;
  esac

  # No prebuilt binary is published for Intel macOS (darwin-amd64); build from source.
  if [ "$os" = "darwin" ] && [ "$arch" = "amd64" ]; then
    fail "No prebuilt binary for Intel macOS (darwin-amd64). Install from source: cargo install --git https://github.com/ReinaMacCredy/maestro --locked"
  fi

  printf "maestro-%s-%s" "$os" "$arch"
}

verify_checksum() {
  local binary_path="$1"
  local checksum_path="$2"
  local asset="$3"
  local expected actual

  expected="$(awk -v asset="$asset" '
    length($1) == 64 && $1 ~ /^[[:xdigit:]]+$/ {
      if (NF == 1 || $2 == asset || $2 == "*" asset) {
        print tolower($1);
        exit;
      }
    }
  ' "$checksum_path")"
  [ -n "$expected" ] || fail "Checksum asset did not contain a SHA-256 digest for $asset."

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$binary_path" | awk '{ print tolower($1) }')"
  else
    actual="$(shasum -a 256 "$binary_path" | awk '{ print tolower($1) }')"
  fi

  [ "$actual" = "$expected" ] || fail "Checksum mismatch for $asset. Refusing to install downloaded binary."
}

main "$@"

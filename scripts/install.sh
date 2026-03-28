#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${MAESTRO_INSTALL_DIR:-$HOME/.local/bin}"

info()  { echo "[ok] $*"; }
warn()  { echo "[--] $*"; }
fail()  { echo "[!!] $*" >&2; exit 1; }

main() {
  echo "maestro installer"
  echo ""

  # 1. Check prerequisites
  command -v bun >/dev/null 2>&1 || fail "bun is required. Install: https://bun.sh"
  command -v git >/dev/null 2>&1 || fail "git is required."
  info "bun $(bun --version)"
  info "git $(git --version | cut -d' ' -f3)"

  # 2. Install CASS if missing
  if command -v cass >/dev/null 2>&1; then
    info "cass already installed: $(cass --version 2>/dev/null || echo 'unknown')"
  else
    echo ""
    echo "CASS (Coding Agent Session Search) is required."
    if command -v brew >/dev/null 2>&1; then
      echo "Installing via Homebrew..."
      brew install dicklesworthstone/tap/cass
      info "cass installed"
    else
      echo ""
      echo "Install CASS manually:"
      echo "  # Homebrew (Apple Silicon macOS + Linux)"
      echo "  brew install dicklesworthstone/tap/cass"
      echo ""
      echo "  # Windows (Scoop)"
      echo "  scoop bucket add cass https://github.com/Dicklesworthstone/coding_agent_session_search"
      echo "  scoop install cass"
      echo ""
      echo "  # Or build from source:"
      echo "  cargo install coding-agent-session-search"
      echo ""
      fail "Install CASS first, then re-run this script."
    fi
  fi

  # 3. Build maestro
  echo ""
  echo "Building maestro..."
  if [ -f "package.json" ] && grep -q "maestro-handoff" package.json 2>/dev/null; then
    bun install --frozen-lockfile
    bun run build
  else
    fail "Run this script from the maestro repository root."
  fi
  info "Built dist/maestro"

  # 4. Install binary
  mkdir -p "$INSTALL_DIR"
  cp ./dist/maestro "$INSTALL_DIR/maestro"
  chmod +x "$INSTALL_DIR/maestro"

  # 5. Verify
  if "$INSTALL_DIR/maestro" --version >/dev/null 2>&1; then
    info "Installed maestro $("$INSTALL_DIR/maestro" --version) to $INSTALL_DIR/maestro"
  else
    fail "Installation verification failed"
  fi

  # 6. Initialize global config + inject agent instructions
  "$INSTALL_DIR/maestro" install --json 2>/dev/null && info "Initialized config and agent instructions" || true

  # 7. PATH hint
  if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo ""
    warn "$INSTALL_DIR is not in your PATH"
    echo "    Add: export PATH=\"$INSTALL_DIR:\$PATH\""
  fi

  # 7. Post-install hint
  echo ""
  echo "Next steps:"
  echo "  maestro init --global    # set up global config"
  echo "  maestro init             # set up project config"
  echo "  maestro doctor           # verify everything works"
}

main "$@"

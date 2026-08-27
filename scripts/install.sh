#!/bin/sh
#   curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install.sh | sh
# Options and environment: install.sh --help
set -eu

case "${1:-}" in
  -h|--help)
    printf '%s\n' \
      'usage: install.sh' \
      '  Clone the maestro source checkout and run its installer for the current repository.' \
      '  MAESTRO_REPO        git URL to clone (default: the GitHub repository)' \
      '  MAESTRO_REF         branch to install and follow (default: main)' \
      '  MAESTRO_SOURCE_DIR  where the checkout lives (default: ~/.maestro/source)'
    exit 0
    ;;
  "") ;;
  *)
    printf 'maestro install: unknown argument %s (try --help)\n' "$1" >&2
    exit 2
    ;;
esac

REPO="${MAESTRO_REPO:-https://github.com/ReinaMacCredy/maestro.git}"
REF="${MAESTRO_REF:-main}"
SOURCE="${MAESTRO_SOURCE_DIR:-$HOME/.maestro/source}"

fail() {
  printf 'maestro install: %s\n' "$1" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required."
command -v bun >/dev/null 2>&1 || fail "bun is required: curl -fsSL https://bun.sh/install | bash"
MIN_BUN="1.4.0"
BUN_VERSION="$(bun --version 2>/dev/null | tr -d '[:space:]')"
bun_at_least() {
  printf '%s\n%s\n' "$1" "$2" | awk -F. 'NR==1{a=$1*1000000+$2*1000+$3} NR==2{b=$1*1000000+$2*1000+$3} END{exit !(b>=a)}'
}
bun_at_least "$MIN_BUN" "$BUN_VERSION" || fail "bun $BUN_VERSION is too old; maestro needs bun >= $MIN_BUN: curl -fsSL https://bun.sh/install | bash"

if [ -d "$SOURCE/.git" ]; then
  printf 'maestro install: fast-forwarding the source checkout at %s\n' "$SOURCE"
  git -C "$SOURCE" pull --ff-only --quiet || fail "the checkout at $SOURCE cannot fast-forward; resolve it or set MAESTRO_SOURCE_DIR."
elif [ -e "$SOURCE" ]; then
  fail "$SOURCE exists and is not a git checkout; set MAESTRO_SOURCE_DIR."
else
  mkdir -p "$(dirname "$SOURCE")"
  printf 'maestro install: cloning %s (%s) into %s\n' "$REPO" "$REF" "$SOURCE"
  git clone --quiet --branch "$REF" "$REPO" "$SOURCE"
fi

bun "$SOURCE/bin/maestro.ts" install

case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) printf 'maestro install: add %s to PATH to run maestro\n' "$HOME/.local/bin" ;;
esac

if ! command -v herdr >/dev/null 2>&1; then
  printf 'maestro install: SLP lanes need Herdr: curl -fsSL https://herdr.dev/install.sh | sh\n'
fi

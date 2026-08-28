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
SOURCE="${MAESTRO_SOURCE_DIR:-$HOME/.maestro/source}"
PINNED_BRANCH="maestro-release"

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

# Pick the newest release by version, not by string order: v0.9.0 sorts above
# v0.10.0 lexicographically, and sort -V is not portable to every /bin/sh host.
newest_release_tag() {
  git ls-remote --tags --refs "$REPO" 2>/dev/null |
    sed 's#.*refs/tags/##' |
    awk -F. '/^v[0-9]+\.[0-9]+\.[0-9]+$/ {
      value = substr($1, 2) * 1000000 + $2 * 1000 + $3
      if (value > best) { best = value; tag = $0 }
    } END { if (tag != "") print tag }'
}

# An adopter gets a release, not whatever landed on main minutes ago. An
# explicit MAESTRO_REF is the development escape hatch and takes the ordinary
# branch path; a repository with no release tags falls back to main so a fork or
# a fresh mirror still installs.
REF="${MAESTRO_REF:-}"
PINNED=""
if [ -z "$REF" ]; then
  REF="$(newest_release_tag)"
  if [ -n "$REF" ]; then
    PINNED=1
  else
    REF="main"
    printf 'maestro install: %s publishes no release tags; installing %s\n' "$REPO" "$REF"
  fi
fi

if [ -d "$SOURCE/.git" ]; then
  printf 'maestro install: fast-forwarding the source checkout at %s\n' "$SOURCE"
  if [ "$(git -C "$SOURCE" symbolic-ref --quiet --short HEAD 2>/dev/null)" = "$PINNED_BRANCH" ]; then
    # A pinned checkout follows the tag line, not a branch tip, and it has no
    # upstream to pull from.
    NEXT="${MAESTRO_REF:-$(newest_release_tag)}"
    [ -n "$NEXT" ] || fail "$REPO publishes no release tags; set MAESTRO_REF to install anyway."
    git -C "$SOURCE" fetch --quiet --tags origin ||
      fail "cannot fetch tags for the checkout at $SOURCE; fix remote connectivity and retry."
    git -C "$SOURCE" merge --ff-only --quiet "$NEXT" ||
      fail "the pinned checkout at $SOURCE cannot fast-forward to $NEXT; resolve it or set MAESTRO_SOURCE_DIR."
  else
    git -C "$SOURCE" pull --ff-only --quiet || fail "the checkout at $SOURCE cannot fast-forward; resolve it or set MAESTRO_SOURCE_DIR."
  fi
elif [ -e "$SOURCE" ]; then
  fail "$SOURCE exists and is not a git checkout; set MAESTRO_SOURCE_DIR."
else
  mkdir -p "$(dirname "$SOURCE")"
  printf 'maestro install: cloning %s (%s) into %s\n' "$REPO" "$REF" "$SOURCE"
  if [ -n "$PINNED" ]; then
    # Land the tag on a branch rather than a detached HEAD: maestro update
    # refuses a detached source, so pinning must not cost the update path.
    git clone --quiet "$REPO" "$SOURCE"
    git -C "$SOURCE" checkout --quiet -b "$PINNED_BRANCH" "$REF"
  else
    git clone --quiet --branch "$REF" "$REPO" "$SOURCE"
  fi
fi

bun "$SOURCE/bin/maestro.ts" install

case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) printf 'maestro install: add %s to PATH to run maestro\n' "$HOME/.local/bin" ;;
esac

if ! command -v herdr >/dev/null 2>&1; then
  printf 'maestro install: SLP lanes need Herdr: curl -fsSL https://herdr.dev/install.sh | sh\n'
fi

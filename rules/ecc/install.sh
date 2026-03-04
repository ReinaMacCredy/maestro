#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_ROOT="${CLAUDE_RULES_DIR:-$HOME/.claude/rules}"

usage() {
  cat <<'USAGE'
Usage: rules/ecc/install.sh <language> [language...]

Installs rules/ecc/common and one or more language rule sets into ~/.claude/rules.

Arguments:
  language  One of: typescript, python, golang, swift

Examples:
  bash rules/ecc/install.sh typescript
  bash rules/ecc/install.sh typescript python
USAGE
}

if [[ $# -eq 0 ]]; then
  usage >&2
  exit 1
fi

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

mkdir -p "$TARGET_ROOT"

copy_rule_dir() {
  local src="$1"
  local dst="$2"
  rm -rf "$dst"
  cp -R "$src" "$dst"
}

copy_rule_dir "$SCRIPT_DIR/common" "$TARGET_ROOT/common"
echo "[ok] installed common rules -> $TARGET_ROOT/common"

for lang in "$@"; do
  case "$lang" in
    typescript|python|golang|swift)
      copy_rule_dir "$SCRIPT_DIR/$lang" "$TARGET_ROOT/$lang"
      echo "[ok] installed $lang rules -> $TARGET_ROOT/$lang"
      ;;
    *)
      echo "[x] Unknown language: $lang" >&2
      usage >&2
      exit 1
      ;;
  esac
done

echo "[ok] ECC rules install complete"

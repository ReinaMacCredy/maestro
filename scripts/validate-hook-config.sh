#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_JSON="$ROOT_DIR/.claude/hooks/hooks.json"

if [[ ! -f "$HOOKS_JSON" ]]; then
  echo "[x] Missing $HOOKS_JSON" >&2
  exit 1
fi

jq -e . "$HOOKS_JSON" >/dev/null

status=0

while IFS= read -r cmd; do
  [[ -z "$cmd" ]] && continue

  while IFS= read -r rel; do
    [[ -z "$rel" ]] && continue
    abs="$ROOT_DIR/$rel"
    if [[ ! -f "$abs" ]]; then
      echo "[x] Missing hook target: $rel (from command: $cmd)" >&2
      status=1
    fi
  done < <(printf '%s\n' "$cmd" | rg -o '\.claude/scripts/[A-Za-z0-9._/-]+|scripts/hooks/ecc/[A-Za-z0-9._/-]+' || true)

done < <(jq -r '.. | objects | select(has("command")) | .command' "$HOOKS_JSON")

if [[ "$status" -eq 0 ]]; then
  echo "[ok] Hook command references validated"
fi

exit "$status"

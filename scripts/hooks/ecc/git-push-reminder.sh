#!/bin/bash
# Stop: reminds when local commits are ahead of upstream.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/ecc/common.sh
source "$SCRIPT_DIR/common.sh"

if ! ecc_quality_gates_enabled; then
  exit 0
fi

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"

if ! git -C "$PROJECT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  exit 0
fi

if ! git -C "$PROJECT_DIR" rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' >/dev/null 2>&1; then
  exit 0
fi

ahead_count="$(git -C "$PROJECT_DIR" rev-list --count '@{upstream}..HEAD' 2>/dev/null || printf '0')"

if [[ "$ahead_count" =~ ^[0-9]+$ ]] && [[ "$ahead_count" -gt 0 ]]; then
  ecc_emit_context "Stop" "ECC reminder: branch is ahead of upstream by $ahead_count commit(s). Run git push when your verification is complete."
fi

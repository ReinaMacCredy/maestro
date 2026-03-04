#!/bin/bash
# PostToolUse(Bash): reminds to run long workflows in tmux.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/ecc/common.sh
source "$SCRIPT_DIR/common.sh"

if ! ecc_quality_gates_enabled; then
  exit 0
fi

if [[ -n "${TMUX:-}" ]]; then
  exit 0
fi

input="$(cat)"
command_text="$(extract_command_text "$input")"

if [[ -z "$command_text" ]]; then
  exit 0
fi

if printf '%s' "$command_text" | grep -qiE '(test|build|lint|typecheck|watch|serve|dev|run-evals|long|benchmark)'; then
  ecc_emit_context "PostToolUse" "ECC reminder: this shell is not in tmux. For long-running commands, use tmux to avoid losing progress on disconnects."
fi

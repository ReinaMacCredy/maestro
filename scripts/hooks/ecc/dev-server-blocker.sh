#!/bin/bash
# PreToolUse(Bash): blocks launching long-running dev servers in this session.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/ecc/common.sh
source "$SCRIPT_DIR/common.sh"

if ! ecc_quality_gates_enabled; then
  exit 0
fi

input="$(cat)"
tool_name="$(extract_tool_name "$input")"
command_text="$(extract_command_text "$input")"

if [[ "$tool_name" != "Bash" || -z "$command_text" ]]; then
  exit 0
fi

if printf '%s' "$command_text" | grep -qiE '(^|[[:space:]])((npm|pnpm|yarn|bun)[[:space:]]+(run[[:space:]]+)?(dev|start|serve|watch)|next[[:space:]]+dev|vite([[:space:]]|$)|webpack-dev-server|astro[[:space:]]+dev)'; then
  ecc_emit_deny "PreToolUse" "ECC quality gate: blocked dev server command ('$command_text'). Run build/test/lint/typecheck only in this agent session, or disable via MAESTRO_ENABLE_ECC_QUALITY_GATES=0 when intentional."
fi

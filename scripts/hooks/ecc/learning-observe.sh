#!/bin/bash
# Optional learning observation hook. Disabled by default.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/ecc/common.sh
source "$SCRIPT_DIR/common.sh"

input="$(cat || true)"

if ! ecc_learning_hooks_enabled; then
  exit 0
fi

event_name="${1:-}"
if [[ -z "$event_name" ]]; then
  event_name="$(printf '%s' "$input" | jq -r '.hook_event_name // .event_name // empty' 2>/dev/null || true)"
fi
if [[ -z "$event_name" ]]; then
  event_name="PreToolUse"
fi

tool_name="$(extract_tool_name "$input")"
if [[ -z "$tool_name" ]]; then
  tool_name="unknown"
fi

ecc_emit_context "$event_name" "ECC learning observe hook enabled. Captured tool event: $tool_name"

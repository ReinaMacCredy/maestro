#!/bin/bash
# PostToolUse(Bash) - detects command failures and injects investigation reminder
set -euo pipefail
input=$(cat)
exit_code=$(printf '%s' "$input" | jq -r '.tool_result.exit_code // "0"' 2>/dev/null) || exit_code="0"
stderr=$(printf '%s' "$input" | jq -r '.tool_result.stderr // empty' 2>/dev/null) || stderr=""
# Check for failure indicators
is_error=false
if [ "$exit_code" != "0" ]; then
  is_error=true
elif printf '%s' "$stderr" | grep -qiE '(error:|Error:|ENOENT|command not found|Permission denied|fatal:|FAILED|panic|Traceback)'; then
  is_error=true
fi
if $is_error; then
  # Truncate stderr to 200 chars for context
  short_err=$(printf '%s' "$stderr" | head -c 200)
  printf '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Command failed (exit %s). Investigate the error before proceeding. Error: %s"}}' "$exit_code" "$short_err"
fi

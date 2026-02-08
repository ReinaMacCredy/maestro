#!/bin/bash
# PostToolUse(Bash) - mirrors successful commands to user's bash history
set -euo pipefail
input=$(cat)
exit_code=$(printf '%s' "$input" | jq -r '.tool_result.exit_code // "0"' 2>/dev/null) || exit_code="0"
command_str=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null) || command_str=""
# Only mirror successful commands
[ "$exit_code" != "0" ] && exit 0
[ -z "$command_str" ] && exit 0
# Skip read-only commands
if printf '%s' "$command_str" | grep -qE '^\s*(cat|ls|grep|head|tail|wc|file|stat|which|type|echo|pwd) '; then
  exit 0
fi
# Skip commands containing secrets
if printf '%s' "$command_str" | grep -qiE '(password|token|secret|key=|api_key|apikey|credential)'; then
  exit 0
fi
# Truncate to 500 chars and append
short_cmd=$(printf '%s' "$command_str" | head -c 500)
printf '%s\n' "$short_cmd" >> ~/.bash_history

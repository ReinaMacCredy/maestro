#!/bin/bash
# PostToolUse(*) - logs tool events to .maestro/trace.jsonl
set -euo pipefail
input=$(cat)
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-}"
[ -z "$PROJECT_DIR" ] && exit 0
trace_file="$PROJECT_DIR/.maestro/trace.jsonl"
mkdir -p "$(dirname "$trace_file")"
tool_name=$(printf '%s' "$input" | jq -r '.tool_name // empty' 2>/dev/null) || tool_name=""
[ -z "$tool_name" ] && exit 0
agent_name="${CLAUDE_AGENT_NAME:-unknown}"
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
exit_code=$(printf '%s' "$input" | jq -r '.tool_result.exit_code // "0"' 2>/dev/null) || exit_code="0"
success="true"
[ "$exit_code" != "0" ] && success="false"
summary=$(printf '%s' "$input" | jq -r '(.tool_input.description // .tool_input.command // .tool_input.pattern // .tool_input.file_path // "") | .[0:200]' 2>/dev/null) || summary=""
printf '{"timestamp":"%s","event_type":"tool_use","tool_name":"%s","agent_name":"%s","success":%s,"summary":"%s"}\n' \
  "$timestamp" "$tool_name" "$agent_name" "$success" "$(printf '%s' "$summary" | sed 's/"/\\"/g' | tr '\n' ' ')" >> "$trace_file"

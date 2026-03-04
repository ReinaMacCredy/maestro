#!/bin/bash
# Shared helpers for ECC hooks.

set -euo pipefail

normalize_bool() {
  local raw="${1:-}"
  printf '%s' "$raw" | tr '[:upper:]' '[:lower:]'
}

is_enabled_with_default() {
  local raw="${1:-}"
  local default_enabled="${2:-0}"
  local normalized

  if [[ -z "$raw" ]]; then
    [[ "$default_enabled" == "1" ]]
    return
  fi

  normalized="$(normalize_bool "$raw")"
  case "$normalized" in
    1|true|yes|on|enable|enabled)
      return 0
      ;;
    0|false|no|off|disable|disabled)
      return 1
      ;;
    *)
      [[ "$default_enabled" == "1" ]]
      return
      ;;
  esac
}

ecc_quality_gates_enabled() {
  is_enabled_with_default "${MAESTRO_ENABLE_ECC_QUALITY_GATES:-}" 1
}

ecc_learning_hooks_enabled() {
  is_enabled_with_default "${MAESTRO_ENABLE_LEARNING_HOOKS:-}" 0
}

ecc_emit_context() {
  local event_name="$1"
  local message="$2"

  printf '%s' "$message" \
    | jq -Rs --arg event_name "$event_name" \
      '{hookSpecificOutput: {hookEventName: $event_name, additionalContext: .}}'
}

ecc_emit_deny() {
  local event_name="$1"
  local reason="$2"

  printf '%s' "$reason" \
    | jq -Rs --arg event_name "$event_name" \
      '{hookSpecificOutput: {hookEventName: $event_name, permissionDecision: "deny", permissionDecisionReason: .}}'
}

extract_tool_name() {
  local input_json="$1"
  printf '%s' "$input_json" \
    | jq -r '.tool_name // .tool.name // .tool_input.tool_name // empty' 2>/dev/null
}

extract_command_text() {
  local input_json="$1"
  printf '%s' "$input_json" \
    | jq -r '.tool_input.command // .tool_input.cmd // .input.command // .command // empty' 2>/dev/null
}

extract_file_path() {
  local input_json="$1"
  printf '%s' "$input_json" \
    | jq -r '.tool_input.file_path // .tool_input.path // .input.file_path // .input.path // empty' 2>/dev/null
}

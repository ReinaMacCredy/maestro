#!/bin/bash
# orchestrator-guard.sh
# Prevents orchestrator agent from directly editing files
# Hook: PreToolUse(Write|Edit)

# Read stdin (hook input)
input=$(cat)

# Determine tool name from hook payload
tool_name=$(echo "$input" | jq -r '
  .tool_name
  // .tool.name
  // .tool_input.tool_name
  // empty
' 2>/dev/null)

# Prefer explicit runtime context first; fall back to minimal hook payload fields.
agent_name="${CLAUDE_AGENT_NAME:-}"
if [[ -z "$agent_name" ]]; then
  agent_name=$(echo "$input" | jq -r '
    .agent_name
    // .agent.name
    // empty
  ' 2>/dev/null)
fi

# Only guard edit tools, and only for orchestrator.
if [[ ("$tool_name" == "Write" || "$tool_name" == "Edit") && "$agent_name" == "orchestrator" ]]; then
  echo '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Orchestrator cannot edit files directly. Delegate to a teammate (kraken, spark) instead."}}'
  exit 0
fi

# Allow other agents to proceed
exit 0

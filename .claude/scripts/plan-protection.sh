#!/bin/bash
# plan-protection.sh
# Blocks kraken/spark from editing .maestro/plans/
# Hook: PreToolUse(Write|Edit)

# Read stdin (hook input)
input=$(cat)

# Extract file path from hook payload
file_path=$(echo "$input" | jq -r '
  .tool_input.file_path
  // .tool_input.path
  // .input.file_path
  // .input.path
  // empty
' 2>/dev/null)

# Only guard .maestro/plans/ files
if [[ "$file_path" != *".maestro/plans/"* ]]; then
  exit 0
fi

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

# Block kraken and spark from editing plans
if [[ "$agent_name" == "kraken" || "$agent_name" == "spark" ]]; then
  echo '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Workers (kraken/spark) cannot edit plan files in .maestro/plans/. Only prometheus and orchestrator can modify plans."}}'
  exit 0
fi

# Allow other agents to proceed
exit 0

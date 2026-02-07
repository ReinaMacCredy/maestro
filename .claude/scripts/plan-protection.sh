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

# Best-effort agent detection from known hook fields
agent_name=$(
  echo "$input" | jq -r '
    .agent_name
    // .agent_type
    // .agent.name
    // .agent.type
    // .caller.agent_name
    // .caller.agent_type
    // .source.agent_name
    // .source.agent_type
    // .metadata.agent_name
    // .metadata.agent_type
    // .tool_input.agent_name
    // .tool_input.agent_type
    // .tool_input.agent.name
    // .tool_input.agent.type
    // empty
  ' 2>/dev/null
)

# Transcript fallback for agent detection
if [[ -z "$agent_name" ]]; then
  transcript_path=$(echo "$input" | jq -r '.transcript_path // empty' 2>/dev/null)
  if [[ -n "$transcript_path" && -r "$transcript_path" ]]; then
    last_event=$(tail -n 1 "$transcript_path")
    if [[ -n "$last_event" ]]; then
      last_event_name=$(echo "$last_event" | jq -r '
        .hook_event_name
        // .event
        // empty
      ' 2>/dev/null)
      last_tool_name=$(echo "$last_event" | jq -r '
        .tool_name
        // .tool.name
        // .tool_input.tool_name
        // empty
      ' 2>/dev/null)
      last_agent_name=$(echo "$last_event" | jq -r '
        .agent_name
        // .agent_type
        // .agent.name
        // .agent.type
        // .caller.agent_name
        // .caller.agent_type
        // .source.agent_name
        // .source.agent_type
        // .metadata.agent_name
        // .metadata.agent_type
        // .tool_input.agent_name
        // .tool_input.agent_type
        // .tool_input.agent.name
        // .tool_input.agent.type
        // empty
      ' 2>/dev/null)

      if [[ "$last_event_name" == "PreToolUse" && "$last_tool_name" == "$tool_name" ]]; then
        agent_name="$last_agent_name"
      fi
    fi
  fi
fi

# Block kraken and spark from editing plans
if [[ "$agent_name" == "kraken" || "$agent_name" == "spark" ]]; then
  echo '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Workers (kraken/spark) cannot edit plan files in .maestro/plans/. Only prometheus and orchestrator can modify plans."}}'
  exit 0
fi

# Allow other agents to proceed
exit 0

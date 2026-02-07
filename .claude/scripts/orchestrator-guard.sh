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

# PreToolUse may omit agent identity. Only use transcript fallback when we can tie
# orchestrator identity to the CURRENT event (the last transcript entry).
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

      if [[ "$last_event_name" == "PreToolUse" && "$last_tool_name" == "$tool_name" && "$last_agent_name" == "orchestrator" ]]; then
        agent_name="orchestrator"
      fi
    fi
  fi
fi

# Only guard edit tools, and only for orchestrator.
if [[ ("$tool_name" == "Write" || "$tool_name" == "Edit") && "$agent_name" == "orchestrator" ]]; then
  echo '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Orchestrator cannot edit files directly. Delegate to a teammate (kraken, spark) instead."}}'
  exit 0
fi

# Allow other agents to proceed
exit 0

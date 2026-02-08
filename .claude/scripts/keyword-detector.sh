#!/bin/bash
# UserPromptSubmit - detects magic keywords and injects mode context
set -euo pipefail
input=$(cat)
# Extract prompt text from message parts
prompt=$(printf '%s' "$input" | jq -r '[.message.content[]? | select(.type=="text") | .text] | join(" ")' 2>/dev/null) || prompt=""
[ -z "$prompt" ] && exit 0
# Strip code blocks to avoid false positives
clean_prompt=$(printf '%s' "$prompt" | sed '/^```/,/^```/d')
context=""
# Check for ecomode keywords
if printf '%s' "$clean_prompt" | grep -qiE '\b(eco|ecomode)\b'; then
  context="[ECOMODE] Use cost-efficient models. Prefer haiku for simple tasks, sonnet for complex."
fi
# Check for ultrawork keywords
if printf '%s' "$clean_prompt" | grep -qiE '\b(ultrawork|ulw)\b'; then
  context="[ULTRAWORK] Maximum thoroughness. Use parallel agents, verify everything, delegate aggressively."
fi
# Check for think keywords
if printf '%s' "$clean_prompt" | grep -qiE '\b(ultrathink|think)\b'; then
  context="[DEEP THINKING] Take extra time to reason through this. Consider multiple approaches, edge cases, and risks before acting."
fi
if [ -n "$context" ]; then
  printf '%s' "$context" | jq -Rs '{hookSpecificOutput: {hookEventName: "UserPromptSubmit", additionalContext: .}}'
fi

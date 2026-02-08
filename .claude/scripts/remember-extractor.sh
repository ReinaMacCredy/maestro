#!/bin/bash
# PostToolUse(Task) - extracts <remember> tags from agent output and appends to wisdom
set -euo pipefail
input=$(cat)
# Extract the tool result text
result_text=$(printf '%s' "$input" | jq -r '.tool_result.stdout // .tool_result.text // empty' 2>/dev/null) || result_text=""
[ -z "$result_text" ] && exit 0

# Check for remember tags
if ! printf '%s' "$result_text" | grep -q '<remember'; then
  exit 0
fi

# Find the active plan
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
wisdom_dir="$PROJECT_DIR/.maestro/wisdom"
mkdir -p "$wisdom_dir"

# Determine active plan name from handoff or default
active_plan="session"
if [ -d "$PROJECT_DIR/.maestro/handoff" ]; then
  for hf in "$PROJECT_DIR/.maestro/handoff"/*.json; do
    [ -f "$hf" ] || continue
    topic=$(jq -r '.topic // empty' "$hf" 2>/dev/null) || continue
    if [ -n "$topic" ]; then
      active_plan=$(printf '%s' "$topic" | tr ' ' '-' | tr '[:upper:]' '[:lower:]')
      break
    fi
  done
fi

wisdom_file="$wisdom_dir/${active_plan}.md"
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Extract all remember tags and append to wisdom
printf '%s' "$result_text" | grep -oE '<remember category="[^"]*">[^<]*</remember>' | while IFS= read -r tag; do
  category=$(printf '%s' "$tag" | sed 's/.*category="\([^"]*\)".*/\1/')
  content=$(printf '%s' "$tag" | sed 's/.*>\(.*\)<\/remember>/\1/')
  # Append under category heading
  if [ -f "$wisdom_file" ] && grep -q "^### ${category^}$" "$wisdom_file" 2>/dev/null; then
    printf '\n- [%s] %s\n' "$timestamp" "$content" >> "$wisdom_file"
  else
    printf '\n### %s\n\n- [%s] %s\n' "${category^}" "$timestamp" "$content" >> "$wisdom_file"
  fi
done

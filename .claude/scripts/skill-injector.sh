#!/bin/bash
# UserPromptSubmit - injects relevant skill descriptions based on prompt keywords
set -euo pipefail
input=$(cat)
prompt=$(printf '%s' "$input" | jq -r '[.message.content[]? | select(.type=="text") | .text] | join(" ")' 2>/dev/null) || prompt=""
[ -z "$prompt" ] && exit 0
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
skills_dir="$PROJECT_DIR/.claude/skills"
[ -d "$skills_dir" ] || exit 0
lower_prompt=$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')
matched_skills=""
for manifest in "$skills_dir"/*/SKILL.md; do
  [ -f "$manifest" ] || continue
  # Parse triggers from YAML frontmatter
  triggers=""
  skill_name=""
  skill_desc=""
  in_frontmatter=false
  while IFS= read -r line; do
    if [ "$line" = "---" ]; then
      if $in_frontmatter; then break; else in_frontmatter=true; continue; fi
    fi
    if $in_frontmatter; then
      case "$line" in
        triggers:*) triggers="${line#triggers:}" ;;
        name:*) skill_name="${line#name: }" ; skill_name="${skill_name#\"}" ; skill_name="${skill_name%\"}" ;;
        description:*) skill_desc="${line#description: }" ; skill_desc="${skill_desc#\"}" ; skill_desc="${skill_desc%\"}" ;;
      esac
    fi
  done < "$manifest"
  [ -z "$triggers" ] && continue
  # Check each trigger against prompt
  for trigger in $(printf '%s' "$triggers" | tr -d '[]",' | tr ' ' '\n'); do
    if printf '%s' "$lower_prompt" | grep -qi "\b${trigger}\b" 2>/dev/null; then
      matched_skills="${matched_skills}Relevant skill: ${skill_name} -- ${skill_desc}. Use /${skill_name} to activate.\n"
      break
    fi
  done
done
if [ -n "$matched_skills" ]; then
  printf '%s' "$matched_skills" | jq -Rs '{hookSpecificOutput: {hookEventName: "UserPromptSubmit", additionalContext: .}}'
fi

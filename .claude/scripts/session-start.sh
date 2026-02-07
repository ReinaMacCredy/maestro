#!/bin/bash
# SessionStart hook - injects Maestro awareness context at session start
# Outputs JSON with available commands, active plans, wisdom, and skills

set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"

# Collect context sections
context_parts=()

# 0. Active plan detection from handoff files
handoff_dir="$PROJECT_DIR/.maestro/handoff"
if [[ -d "$handoff_dir" ]]; then
  for handoff in "$handoff_dir"/*.json; do
    [[ -f "$handoff" ]] || continue
    status=$(jq -r '.status // empty' "$handoff" 2>/dev/null) || continue
    topic=$(jq -r '.topic // empty' "$handoff" 2>/dev/null) || continue
    case "$status" in
      executing)
        context_parts+=("ACTIVE PLAN: $topic (status: executing) — Run /work --resume to continue")
        ;;
      designing)
        context_parts+=("ACTIVE PLAN: $topic (status: designing) — Run /design to continue")
        ;;
    esac
  done
fi

# 1. Available Maestro commands (always present)
context_parts+=("Maestro commands: /design, /work, /status, /review, /reset, /plan-template")

# 1.5 Project context availability
context_dir="$PROJECT_DIR/.maestro/context"
if [[ -d "$context_dir" ]]; then
  ctx_count=0
  for cfile in "$context_dir"/*.md; do
    [[ -f "$cfile" ]] || continue
    ctx_count=$((ctx_count + 1))
  done
  if [[ $ctx_count -gt 0 ]]; then
    context_parts+=("Project context: $ctx_count files (.maestro/context/) — run /setup to update")
  fi
fi

# 2. Skills - parse name and description from YAML frontmatter
skills_dir="$PROJECT_DIR/.claude/skills"
if [[ -d "$skills_dir" ]]; then
  skills=""
  for manifest in "$skills_dir"/*/SKILL.md; do
    [[ -f "$manifest" ]] || continue
    skill_name=""
    skill_desc=""
    in_frontmatter=false
    while IFS= read -r line; do
      if [[ "$line" == "---" ]]; then
        if $in_frontmatter; then
          break
        else
          in_frontmatter=true
          continue
        fi
      fi
      if $in_frontmatter; then
        if [[ "$line" =~ ^name:\ *(.*) ]]; then
          skill_name="${BASH_REMATCH[1]}"
          # Strip surrounding quotes if present
          skill_name="${skill_name#\"}"
          skill_name="${skill_name%\"}"
        elif [[ "$line" =~ ^description:\ *(.*) ]]; then
          skill_desc="${BASH_REMATCH[1]}"
          skill_desc="${skill_desc#\"}"
          skill_desc="${skill_desc%\"}"
        fi
      fi
    done < "$manifest"
    if [[ -n "$skill_name" ]]; then
      if [[ -n "$skills" ]]; then
        skills="$skills; $skill_name: $skill_desc"
      else
        skills="$skill_name: $skill_desc"
      fi
    fi
  done
  if [[ -n "$skills" ]]; then
    context_parts+=("Skills: $skills")
  fi
fi

# 3. Active plans - name and first-line title
plans_dir="$PROJECT_DIR/.maestro/plans"
if [[ -d "$plans_dir" ]]; then
  plans=""
  for plan in "$plans_dir"/*.md; do
    [[ -f "$plan" ]] || continue
    basename_plan="$(basename "$plan")"
    [[ "$basename_plan" == ".gitkeep" ]] && continue
    plan_name="${basename_plan%.md}"
    # Read first non-empty line as title (strip leading #)
    title=""
    while IFS= read -r line; do
      line="${line#"${line%%[! ]*}"}"  # trim leading whitespace
      [[ -z "$line" ]] && continue
      title="${line#\# }"
      break
    done < "$plan"
    if [[ -n "$plans" ]]; then
      plans="$plans; $plan_name ($title)"
    else
      plans="$plan_name ($title)"
    fi
  done
  if [[ -n "$plans" ]]; then
    context_parts+=("Active plans: $plans")
  fi
fi

# 4. Wisdom files - name and first-line title
wisdom_dir="$PROJECT_DIR/.maestro/wisdom"
if [[ -d "$wisdom_dir" ]]; then
  wisdom=""
  for wfile in "$wisdom_dir"/*.md; do
    [[ -f "$wfile" ]] || continue
    basename_w="$(basename "$wfile")"
    [[ "$basename_w" == ".gitkeep" ]] && continue
    w_name="${basename_w%.md}"
    title=""
    while IFS= read -r line; do
      line="${line#"${line%%[! ]*}"}"
      [[ -z "$line" ]] && continue
      title="${line#\# }"
      break
    done < "$wfile"
    if [[ -n "$wisdom" ]]; then
      wisdom="$wisdom; $w_name ($title)"
    else
      wisdom="$w_name ($title)"
    fi
  done
  if [[ -n "$wisdom" ]]; then
    context_parts+=("Wisdom: $wisdom")
  fi
fi

# If only the static commands line exists and nothing else was found, exit silently
if [[ ${#context_parts[@]} -le 1 ]]; then
  # Check if skills/plans/wisdom dirs even exist with content
  has_content=false
  [[ -d "$skills_dir" ]] && compgen -G "$skills_dir/*/SKILL.md" > /dev/null 2>&1 && has_content=true
  [[ -d "$plans_dir" ]] && compgen -G "$plans_dir/*.md" > /dev/null 2>&1 && has_content=true
  [[ -d "$wisdom_dir" ]] && compgen -G "$wisdom_dir/*.md" > /dev/null 2>&1 && has_content=true
  if ! $has_content; then
    exit 0
  fi
fi

# Build the combined context string
combined=""
for part in "${context_parts[@]}"; do
  if [[ -n "$combined" ]]; then
    combined="$combined\n$part"
  else
    combined="$part"
  fi
done

# Output JSON
printf '%s' "$combined" | jq -Rs '{hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: .}}'

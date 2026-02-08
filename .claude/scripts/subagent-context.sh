#!/bin/bash
# SubagentStart hook - injects plan and wisdom context for Maestro worker agents
# Reads hook input from stdin, checks agent_type, and outputs context JSON

set -euo pipefail

# Read stdin
input="$(cat)"

# Extract agent_type from hook input
agent_type="$(printf '%s' "$input" | jq -r '.agent_type // empty' 2>/dev/null)" || true

# Only inject context for known Maestro worker agents
case "$agent_type" in
  kraken|spark|build-fixer|critic|explore|oracle|leviathan|wisdom-synthesizer|progress-reporter|security-reviewer)
    ;;
  *)
    # Not a Maestro agent — exit silently
    exit 0
    ;;
esac

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"

context_parts=()

# 1. Active plan summary - title + task list from first active plan
plans_dir="$PROJECT_DIR/.maestro/plans"
if [[ -d "$plans_dir" ]]; then
  for plan in "$plans_dir"/*.md; do
    [[ -f "$plan" ]] || continue
    basename_plan="$(basename "$plan")"
    [[ "$basename_plan" == ".gitkeep" ]] && continue

    # Read title (first non-empty line, strip #)
    title=""
    while IFS= read -r line; do
      line="${line#"${line%%[! ]*}"}"
      [[ -z "$line" ]] && continue
      title="${line#\# }"
      break
    done < "$plan"

    # Extract task lines (lines matching "- [ ]" or "- [x]")
    tasks=""
    while IFS= read -r line; do
      trimmed="${line#"${line%%[! ]*}"}"
      if [[ "$trimmed" =~ ^-\ \[.\] ]]; then
        if [[ -n "$tasks" ]]; then
          tasks="$tasks\n  $trimmed"
        else
          tasks="  $trimmed"
        fi
      fi
    done < "$plan"

    plan_summary="Active plan: $title"
    if [[ -n "$tasks" ]]; then
      plan_summary="$plan_summary\nTasks:\n$tasks"
    fi
    context_parts+=("$plan_summary")
    break  # Only first active plan
  done
fi

# 2. Wisdom file titles
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

# 3. Project context file titles
context_dir="$PROJECT_DIR/.maestro/context"
if [[ -d "$context_dir" ]]; then
  pctx=""
  for cfile in "$context_dir"/*.md; do
    [[ -f "$cfile" ]] || continue
    basename_c="$(basename "$cfile")"
    c_name="${basename_c%.md}"
    title=""
    while IFS= read -r line; do
      line="${line#"${line%%[! ]*}"}"
      [[ -z "$line" ]] && continue
      title="${line#\# }"
      break
    done < "$cfile"
    if [[ -n "$pctx" ]]; then
      pctx="$pctx; $c_name ($title)"
    else
      pctx="$c_name ($title)"
    fi
  done
  if [[ -n "$pctx" ]]; then
    context_parts+=("Project context: $pctx")
  fi
fi

# If no context was gathered, exit silently
if [[ ${#context_parts[@]} -eq 0 ]]; then
  exit 0
fi

# Build combined context string
combined=""
for part in "${context_parts[@]}"; do
  if [[ -n "$combined" ]]; then
    combined="$combined\n$part"
  else
    combined="$part"
  fi
done

# Output JSON
printf '%s' "$combined" | jq -Rs '{hookSpecificOutput: {hookEventName: "SubagentStart", additionalContext: .}}'

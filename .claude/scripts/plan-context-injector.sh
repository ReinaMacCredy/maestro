#!/bin/bash
# plan-context-injector.sh
# PreCompact hook - injects active plan context into compaction summary
# so the active plan name survives /compact and auto-compact.
#
# PreCompact hooks: stdout is appended to the system prompt for the compact call.
# Exit 0 with empty stdout = proceed normally.
# Exit 0 with stdout = append content to compact system prompt.

set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
HANDOFF_DIR="$PROJECT_DIR/.maestro/handoff"

# Exit silently if no handoff directory
[[ -d "$HANDOFF_DIR" ]] || exit 0

active_plans=""

for handoff in "$HANDOFF_DIR"/*.json; do
  [[ -f "$handoff" ]] || continue
  status=$(jq -r '.status // empty' "$handoff" 2>/dev/null) || continue
  topic=$(jq -r '.topic // empty' "$handoff" 2>/dev/null) || continue
  plan_dest=$(jq -r '.plan_destination // empty' "$handoff" 2>/dev/null) || continue

  case "$status" in
    executing)
      if [[ -n "$active_plans" ]]; then
        active_plans="$active_plans\n- EXECUTING plan: $topic (file: $plan_dest)"
      else
        active_plans="- EXECUTING plan: $topic (file: $plan_dest)"
      fi
      ;;
    designing)
      if [[ -n "$active_plans" ]]; then
        active_plans="$active_plans\n- DESIGNING plan: $topic (file: $plan_dest)"
      else
        active_plans="- DESIGNING plan: $topic (file: $plan_dest)"
      fi
      ;;
  esac
done

# Exit silently if no active plans
[[ -z "$active_plans" ]] && exit 0

# Output context for the compact system prompt
printf 'IMPORTANT — Active Maestro plan context (preserve in summary):\n%b\nThe user may want to resume this plan after compaction. Retain the plan name and status in the summary.' "$active_plans"

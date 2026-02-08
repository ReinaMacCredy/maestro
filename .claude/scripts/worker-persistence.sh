#!/bin/bash
# Stop hook - prevents worker agents from stopping while tasks remain
# Hook: Stop
set -euo pipefail
input=$(cat)
agent_type=$(printf '%s' "$input" | jq -r '.agent_type // empty' 2>/dev/null) || true
# Only intercept Maestro worker agents
case "$agent_type" in
  kraken|spark|build-fixer) ;;
  *) exit 0 ;;
esac
# Check iteration count (OPTIONAL -- set by orchestrator via env)
# If not set, default to 0 (will not trigger max-iteration exit)
max_iterations="${MAESTRO_MAX_ITERATIONS:-10}"
iteration="${MAESTRO_ITERATION:-0}"
if [ "$iteration" -ge "$max_iterations" ]; then
  exit 0  # Allow stop after max iterations
fi
# Check session staleness (OPTIONAL -- set by orchestrator via env)
# If not set, skip staleness check (safe default: keep blocking)
session_start="${MAESTRO_SESSION_START:-}"
if [ -n "$session_start" ]; then
  now=$(date +%s)
  elapsed=$(( now - session_start ))
  if [ "$elapsed" -gt 7200 ]; then
    exit 0  # Allow stop after 2 hours
  fi
fi
# Block stop -- worker should keep going
cat <<'EOF'
{"decision":"block","reason":"Tasks may remain incomplete. Continue working -- use TaskList() to find remaining tasks."}
EOF

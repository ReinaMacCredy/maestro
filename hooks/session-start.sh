#!/usr/bin/env bash
# SessionStart hook for Maestro plugin

set -euo pipefail

# Output minimal context injection as JSON
# Skills are now loaded via Claude Code's native skill system
cat <<EOF
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "<EXTREMELY_IMPORTANT>\nYou have Maestro skills available. Use the 'Skill' tool to load skills when needed.\n\nKey skills: conductor, design (ds), beads, orchestrator.\nSee AGENTS.md for workflow commands.\n</EXTREMELY_IMPORTANT>"
  }
}
EOF

exit 0

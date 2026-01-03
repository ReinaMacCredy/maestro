#!/bin/bash
# amp-session.sh - Session wrapper with auto-restart support
#
# Usage: ./scripts/amp-session.sh [directory]
#
# When Claude exits with code 42, the session auto-restarts.
# This enables the /handoff command to trigger a fresh session.

set -e

WORK_DIR="${1:-$(pwd)}"
HANDOFF_EXIT_CODE=42

cd "$WORK_DIR"

echo "🚀 Starting Amp session in: $WORK_DIR"
echo "   Exit code $HANDOFF_EXIT_CODE triggers auto-restart"
echo ""

while true; do
    # Run amp with initial prime command if handoff exists
    if [ -f "conductor/.handoff_pending" ]; then
        echo "📋 Found pending handoff, loading context..."
        amp --prompt "/conductor-handoff resume"
        rm -f "conductor/.handoff_pending"
    else
        amp
    fi
    
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -eq $HANDOFF_EXIT_CODE ]; then
        echo ""
        echo "🔄 Handoff requested. Restarting fresh session..."
        echo ""
        # Create marker for next session
        touch "conductor/.handoff_pending"
        sleep 1
        continue
    else
        echo ""
        echo "👋 Session ended (exit code: $EXIT_CODE)"
        break
    fi
done

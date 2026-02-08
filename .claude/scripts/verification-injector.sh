#!/bin/bash
# verification-injector.sh
# Reminds to verify task results after delegation
# Hook: PostToolUse(Task)

# Read and discard stdin (hook input not needed for static injection)
cat > /dev/null

# Inject reminder message
cat << 'EOF'
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"VERIFICATION REQUIRED: Read files claimed modified, run tests claimed to pass, and check for errors. Evidence older than 5 minutes is STALE -- re-run verification commands for fresh output."}}
EOF

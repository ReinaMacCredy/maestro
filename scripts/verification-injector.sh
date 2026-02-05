#!/bin/bash
# verification-injector.sh
# Reminds to verify task results after delegation
# Hook: PostToolUse(Task)

# Read stdin (hook input)
input=$(cat)

# Inject reminder message
cat << 'EOF'
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Task completed. Remember to VERIFY: Read files claimed modified, run tests claimed to pass, and check for errors."}}
EOF

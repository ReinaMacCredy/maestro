#!/bin/bash
# plan-validator.sh
# Warns if plan file missing required sections
# Hook: PostToolUse(Write)

# Read stdin (hook input)
input=$(cat)

# Extract the file path from the hook payload
file_path=$(echo "$input" | jq -r '
  .tool_input.file_path
  // .tool_input.path
  // .input.file_path
  // .input.path
  // empty
' 2>/dev/null)

# Only check files written to .maestro/plans/
if [[ "$file_path" != *".maestro/plans/"* ]]; then
  exit 0
fi

# Check if the file exists
if [[ ! -f "$file_path" ]]; then
  exit 0
fi

# Required sections in a plan file
missing=()
for section in "## Objective" "## Scope" "## Tasks" "## Verification"; do
  if ! grep -q "$section" "$file_path" 2>/dev/null; then
    missing+=("$section")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  missing_list=$(printf ', %s' "${missing[@]}")
  missing_list=${missing_list:2}
  cat << EOF
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Plan file is missing required sections: ${missing_list}. A complete plan should include ## Objective, ## Scope, ## Tasks, and ## Verification sections."}}
EOF
fi

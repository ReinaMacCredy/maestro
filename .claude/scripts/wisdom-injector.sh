#!/bin/bash
# wisdom-injector.sh
# Lists wisdom files when a plan is read
# Hook: PostToolUse(Read)

# Read stdin (hook input)
input=$(cat)

# Extract file path from hook payload
file_path=$(echo "$input" | jq -r '
  .tool_input.file_path
  // .tool_input.path
  // .input.file_path
  // .input.path
  // empty
' 2>/dev/null)

# Only trigger when reading plan files
if [[ "$file_path" != *".maestro/plans/"* ]]; then
  exit 0
fi

# Find wisdom files
wisdom_dir="$CLAUDE_PROJECT_DIR"/.maestro/wisdom
if [[ ! -d "$wisdom_dir" ]]; then
  exit 0
fi

# Build list of wisdom files with titles
wisdom_list=""
for f in "$wisdom_dir"/*.md; do
  [[ -f "$f" ]] || continue
  title=$(head -n 1 "$f" | sed 's/^#* *//')
  if [[ -n "$wisdom_list" ]]; then
    wisdom_list="$wisdom_list\n- ${f}: ${title}"
  else
    wisdom_list="\n- ${f}: ${title}"
  fi
done

if [[ -z "$wisdom_list" ]]; then
  exit 0
fi

printf '%s' "Wisdom files available for this project:${wisdom_list}\nConsider reading relevant wisdom files before starting work." \
  | jq -Rs '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: .}}'

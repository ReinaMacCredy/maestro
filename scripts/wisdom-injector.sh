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
wisdom_dir=".maestro/wisdom"
if [[ ! -d "$wisdom_dir" ]]; then
  exit 0
fi

wisdom_files=$(ls "$wisdom_dir"/*.md 2>/dev/null)
if [[ -z "$wisdom_files" ]]; then
  exit 0
fi

# Build list of wisdom files with titles
wisdom_list=""
for f in $wisdom_files; do
  title=$(head -n 1 "$f" | sed 's/^#* *//')
  wisdom_list="${wisdom_list}\n- ${f}: ${title}"
done

cat << EOF
{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Wisdom files available for this project:${wisdom_list}\nConsider reading relevant wisdom files before starting work."}}
EOF

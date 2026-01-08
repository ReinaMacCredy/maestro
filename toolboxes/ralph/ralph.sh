#!/bin/bash
# Ralph Wiggum - Long-running AI agent loop
# Usage: ./ralph.sh <track-path> [max_iterations]

set -e

# Required: track path argument
TRACK_PATH=${1:?Usage: ralph.sh <track-path> [max_iterations]}
MAX_ITERATIONS=${2:-10}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
METADATA_FILE="$TRACK_PATH/metadata.json"
PROGRESS_FILE="$TRACK_PATH/progress.txt"
ARCHIVE_DIR="$SCRIPT_DIR/archive"
LAST_BRANCH_FILE="$SCRIPT_DIR/.last-branch"

# Validate track path and metadata file
if [ ! -d "$TRACK_PATH" ]; then
  echo "Error: Track path does not exist: $TRACK_PATH"
  exit 1
fi

if [ ! -f "$METADATA_FILE" ]; then
  echo "Error: metadata.json not found: $METADATA_FILE"
  exit 1
fi

# Cleanup function to release lock on exit
cleanup() {
  echo "Releasing ralph.active lock..."
  jq '.ralph.active = false' "$METADATA_FILE" > "$METADATA_FILE.tmp" && mv "$METADATA_FILE.tmp" "$METADATA_FILE"
}
trap cleanup EXIT

# Archive previous run if branch changed
if [ -f "$LAST_BRANCH_FILE" ]; then
  CURRENT_BRANCH=$(jq -r '.workflow.branch // empty' "$METADATA_FILE" 2>/dev/null || echo "")
  LAST_BRANCH=$(cat "$LAST_BRANCH_FILE" 2>/dev/null || echo "")
  
  if [ -n "$CURRENT_BRANCH" ] && [ -n "$LAST_BRANCH" ] && [ "$CURRENT_BRANCH" != "$LAST_BRANCH" ]; then
    # Archive the previous run
    DATE=$(date +%Y-%m-%d)
    # Strip "ralph/" prefix from branch name for folder
    FOLDER_NAME=$(echo "$LAST_BRANCH" | sed 's|^ralph/||')
    ARCHIVE_FOLDER="$ARCHIVE_DIR/$DATE-$FOLDER_NAME"
    
    echo "Archiving previous run: $LAST_BRANCH"
    mkdir -p "$ARCHIVE_FOLDER"
    [ -f "$PROGRESS_FILE" ] && cp "$PROGRESS_FILE" "$ARCHIVE_FOLDER/"
    echo "   Archived to: $ARCHIVE_FOLDER"
    
    # Reset progress file for new run
    echo "# Ralph Progress Log" > "$PROGRESS_FILE"
    echo "Started: $(date)" >> "$PROGRESS_FILE"
    echo "Track: $TRACK_PATH" >> "$PROGRESS_FILE"
    echo "---" >> "$PROGRESS_FILE"
  fi
fi

# Track current branch
CURRENT_BRANCH=$(jq -r '.workflow.branch // empty' "$METADATA_FILE" 2>/dev/null || echo "")
if [ -n "$CURRENT_BRANCH" ]; then
  echo "$CURRENT_BRANCH" > "$LAST_BRANCH_FILE"
fi

# Initialize progress file if it doesn't exist
if [ ! -f "$PROGRESS_FILE" ]; then
  echo "# Ralph Progress Log" > "$PROGRESS_FILE"
  echo "Started: $(date)" >> "$PROGRESS_FILE"
  echo "Track: $TRACK_PATH" >> "$PROGRESS_FILE"
  echo "---" >> "$PROGRESS_FILE"
fi

# Set ralph.active lock and reset iteration counter
echo "Setting ralph.active lock..."
jq '.ralph.active = true | .ralph.currentIteration = 0' "$METADATA_FILE" > "$METADATA_FILE.tmp" && mv "$METADATA_FILE.tmp" "$METADATA_FILE"

# Get story count for progress display
STORY_COUNT=$(jq '.ralph.stories | length' "$METADATA_FILE" 2>/dev/null || echo "0")
echo "Starting Ralph - Max iterations: $MAX_ITERATIONS, Stories: $STORY_COUNT"
echo "Track: $TRACK_PATH"

for i in $(seq 1 $MAX_ITERATIONS); do
  echo ""
  echo "═══════════════════════════════════════════════════════"
  echo "  Ralph Iteration $i of $MAX_ITERATIONS"
  echo "═══════════════════════════════════════════════════════"
  
  # Update current iteration in metadata
  jq --argjson iter "$i" '.ralph.currentIteration = $iter' "$METADATA_FILE" > "$METADATA_FILE.tmp" && mv "$METADATA_FILE.tmp" "$METADATA_FILE"
  
  # Run amp with the ralph prompt, injecting track path into prompt
  OUTPUT=$(sed "s|\$TRACK_PATH|$TRACK_PATH|g" "$SCRIPT_DIR/prompt.md" | amp --dangerously-allow-all 2>&1 | tee /dev/stderr) || true
  
  # Log iteration to progress file
  echo "" >> "$PROGRESS_FILE"
  echo "## Iteration $i - $(date)" >> "$PROGRESS_FILE"
  
  # Check for completion signal
  if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
    echo ""
    echo "Ralph completed all tasks!"
    echo "Completed at iteration $i of $MAX_ITERATIONS"
    
    # Log completion
    echo "STATUS: COMPLETE" >> "$PROGRESS_FILE"
    
    # Set all stories to passes = true on completion
    jq '.ralph.stories = (.ralph.stories | map_values(.passes = true))' "$METADATA_FILE" > "$METADATA_FILE.tmp" && mv "$METADATA_FILE.tmp" "$METADATA_FILE"
    
    exit 0
  fi
  
  # Check for story completion markers and update passes status
  # Format: <story-complete>STORY_ID</story-complete>
  # Note: Using sed for macOS compatibility (no grep -P)
  COMPLETED_STORIES=$(echo "$OUTPUT" | sed -n 's/.*<story-complete>\([^<]*\)<\/story-complete>.*/\1/p' || true)
  if [ -n "$COMPLETED_STORIES" ]; then
    for STORY_ID in $COMPLETED_STORIES; do
      echo "Marking story complete: $STORY_ID"
      jq --arg id "$STORY_ID" '.ralph.stories[$id].passes = true' "$METADATA_FILE" > "$METADATA_FILE.tmp" && mv "$METADATA_FILE.tmp" "$METADATA_FILE"
      echo "  - Story $STORY_ID: PASSED" >> "$PROGRESS_FILE"
    done
  fi
  
  echo "Iteration $i complete. Continuing..."
  sleep 2
done

echo ""
echo "Ralph reached max iterations ($MAX_ITERATIONS) without completing all tasks."
echo "Check $PROGRESS_FILE for status."

# Log timeout
echo "" >> "$PROGRESS_FILE"
echo "STATUS: TIMEOUT (max iterations reached)" >> "$PROGRESS_FILE"

exit 1

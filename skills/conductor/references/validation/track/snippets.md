# Track Validation Snippets

Bash code templates for state file operations. Use with atomic write pattern.

## State File Templates

### metadata.json

```json
{
  "track_id": "${TRACK_ID}",
  "type": "feature",
  "status": "new",
  "created_at": "${ISO_TIMESTAMP}",
  "updated_at": "${ISO_TIMESTAMP}",
  "description": "",
  "repairs": []
}
```

**Create command:**

```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
cat > "$TRACK_DIR/metadata.json.tmp.$$" << EOF
{
  "track_id": "$TRACK_ID",
  "type": "feature",
  "status": "new",
  "created_at": "$NOW",
  "updated_at": "$NOW",
  "description": "",
  "repairs": []
}
EOF
mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
```

### .track-progress.json

```json
{
  "trackId": "${TRACK_ID}",
  "status": "complete",
  "specCreatedAt": "${SPEC_MTIME_ISO}",
  "planCreatedAt": "${PLAN_MTIME_ISO}",
  "threadId": "${THREAD_ID}",
  "createdAt": "${ISO_TIMESTAMP}",
  "updatedAt": "${ISO_TIMESTAMP}"
}
```

**Create command:**

```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Get file modification times (macOS)
SPEC_MTIME=$(stat -f %m "$TRACK_DIR/spec.md" 2>/dev/null)
PLAN_MTIME=$(stat -f %m "$TRACK_DIR/plan.md" 2>/dev/null)

# Convert to ISO (macOS)
SPEC_ISO=$(date -r $SPEC_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")
PLAN_ISO=$(date -r $PLAN_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")

# Get thread ID from environment or use null
THREAD_ID="${AMP_THREAD_ID:-null}"

cat > "$TRACK_DIR/.track-progress.json.tmp.$$" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "complete",
  "specCreatedAt": "$SPEC_ISO",
  "planCreatedAt": "$PLAN_ISO",
  "threadId": $([[ "$THREAD_ID" == "null" ]] && echo "null" || echo "\"$THREAD_ID\""),
  "createdAt": "$NOW",
  "updatedAt": "$NOW"
}
EOF
mv "$TRACK_DIR/.track-progress.json.tmp.$$" "$TRACK_DIR/.track-progress.json"
```

### .fb-progress.json

```json
{
  "trackId": "${TRACK_ID}",
  "status": "pending",
  "startedAt": null,
  "completedAt": null,
  "threadId": null,
  "resumeFrom": "phase1",
  "epics": [],
  "issues": [],
  "crossTrackDeps": [],
  "lastBatchCompleted": null,
  "lastError": null
}
```

**Create command:**

```bash
cat > "$TRACK_DIR/.fb-progress.json.tmp.$$" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "pending",
  "startedAt": null,
  "completedAt": null,
  "threadId": null,
  "resumeFrom": "phase1",
  "epics": [],
  "issues": [],
  "crossTrackDeps": [],
  "lastBatchCompleted": null,
  "lastError": null
}
EOF
mv "$TRACK_DIR/.fb-progress.json.tmp.$$" "$TRACK_DIR/.fb-progress.json"
```

## Atomic Write Pattern

Always use temp file + rename to prevent corruption:

```bash
# ❌ WRONG - Can leave partial file on interrupt
echo '{"key": "value"}' > "$FILE"

# ✅ CORRECT - Atomic operation
echo '{"key": "value"}' > "$FILE.tmp.$$"
mv "$FILE.tmp.$$" "$FILE"
```

For jq updates:

```bash
# ❌ WRONG - Truncates file before reading complete
jq '.key = "value"' "$FILE" > "$FILE"

# ✅ CORRECT - Read complete, then atomic write
jq '.key = "value"' "$FILE" > "$FILE.tmp.$$" && mv "$FILE.tmp.$$" "$FILE"
```

## Repair Log Entry

Add repair to `metadata.json.repairs[]` (keep last 10):

```bash
add_repair_log() {
  local FILE="$1"
  local ACTION="$2"
  local FIELD="$3"
  local FROM_VAL="$4"
  local TO_VAL="$5"
  local BY="${AMP_THREAD_ID:-manual}"
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  jq --arg ts "$NOW" \
     --arg action "$ACTION" \
     --arg field "$FIELD" \
     --arg from "$FROM_VAL" \
     --arg to "$TO_VAL" \
     --arg by "$BY" \
     '.repairs = ([{
       timestamp: $ts,
       action: $action,
       field: $field,
       from: $from,
       to: $to,
       by: $by
     }] + (.repairs // []))[:10]' \
     "$FILE" > "$FILE.tmp.$$" && mv "$FILE.tmp.$$" "$FILE"
}

# Usage:
add_repair_log "$TRACK_DIR/metadata.json" "auto-fix" "track_id" "old-id" "new-id"
```

## Auto-Create All State Files

Combined script for creating all missing state files:

```bash
auto_create_state_files() {
  local TRACK_DIR="$1"
  local TRACK_ID=$(basename "$TRACK_DIR")
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # metadata.json
  if [[ ! -f "$TRACK_DIR/metadata.json" ]]; then
    echo "Creating metadata.json for $TRACK_ID"
    cat > "$TRACK_DIR/metadata.json.tmp.$$" << EOF
{
  "track_id": "$TRACK_ID",
  "type": "feature",
  "status": "new",
  "created_at": "$NOW",
  "updated_at": "$NOW",
  "description": "Auto-created by validation",
  "repairs": [{
    "timestamp": "$NOW",
    "action": "auto-create",
    "field": "file",
    "from": null,
    "to": "metadata.json",
    "by": "${AMP_THREAD_ID:-validation}"
  }]
}
EOF
    mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
  fi
  
  # .track-progress.json
  if [[ ! -f "$TRACK_DIR/.track-progress.json" ]]; then
    echo "Creating .track-progress.json for $TRACK_ID"
    # Get file times
    SPEC_MTIME=$(stat -f %m "$TRACK_DIR/spec.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/spec.md")
    PLAN_MTIME=$(stat -f %m "$TRACK_DIR/plan.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/plan.md")
    SPEC_ISO=$(date -r $SPEC_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")
    PLAN_ISO=$(date -r $PLAN_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")
    
    cat > "$TRACK_DIR/.track-progress.json.tmp.$$" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "complete",
  "specCreatedAt": "$SPEC_ISO",
  "planCreatedAt": "$PLAN_ISO",
  "threadId": null,
  "createdAt": "$NOW",
  "updatedAt": "$NOW"
}
EOF
    mv "$TRACK_DIR/.track-progress.json.tmp.$$" "$TRACK_DIR/.track-progress.json"
  fi
  
  # .fb-progress.json
  if [[ ! -f "$TRACK_DIR/.fb-progress.json" ]]; then
    echo "Creating .fb-progress.json for $TRACK_ID"
    cat > "$TRACK_DIR/.fb-progress.json.tmp.$$" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "pending",
  "startedAt": null,
  "completedAt": null,
  "threadId": null,
  "resumeFrom": "phase1",
  "epics": [],
  "issues": [],
  "crossTrackDeps": [],
  "lastBatchCompleted": null,
  "lastError": null
}
EOF
    mv "$TRACK_DIR/.fb-progress.json.tmp.$$" "$TRACK_DIR/.fb-progress.json"
  fi
}

# Usage:
auto_create_state_files "conductor/tracks/my-track_20251224"
```

## Update State File Status

```bash
update_track_status() {
  local TRACK_DIR="$1"
  local NEW_STATUS="$2"
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Update metadata.json
  if [[ -f "$TRACK_DIR/metadata.json" ]]; then
    jq --arg status "$NEW_STATUS" --arg now "$NOW" \
       '.status = $status | .updated_at = $now' \
       "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp.$$"
    mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
  fi
}

# Usage:
update_track_status "conductor/tracks/my-track_20251224" "in_progress"
```

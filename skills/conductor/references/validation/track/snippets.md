# Track Validation Snippets

Bash code templates for metadata.json operations. Use with atomic write pattern.

## State File Template

### metadata.json (Consolidated)

Contains all track state including `generation` and `beads` sections.

```json
{
  "track_id": "${TRACK_ID}",
  "type": "feature",
  "status": "new",
  "created_at": "${ISO_TIMESTAMP}",
  "updated_at": "${ISO_TIMESTAMP}",
  "description": "",
  "priority": "medium",
  "generation": {
    "status": "initializing",
    "specCreatedAt": null,
    "planCreatedAt": null,
    "rbCompletedAt": null
  },
  "beads": {
    "status": "pending",
    "epicId": null,
    "epics": [],
    "issues": [],
    "planTasks": {},
    "beadToTask": {},
    "crossTrackDeps": [],
    "reviewStatus": null,
    "reviewedAt": null
  },
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
  "priority": "medium",
  "generation": {
    "status": "initializing",
    "specCreatedAt": null,
    "planCreatedAt": null,
    "rbCompletedAt": null
  },
  "beads": {
    "status": "pending",
    "epicId": null,
    "epics": [],
    "issues": [],
    "planTasks": {},
    "beadToTask": {},
    "crossTrackDeps": [],
    "reviewStatus": null,
    "reviewedAt": null
  },
  "repairs": []
}
EOF
mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
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

## Auto-Create metadata.json

Create metadata.json with generation and beads sections initialized:

```bash
auto_create_metadata() {
  local TRACK_DIR="$1"
  local TRACK_ID=$(basename "$TRACK_DIR")
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  if [[ ! -f "$TRACK_DIR/metadata.json" ]]; then
    echo "Creating metadata.json for $TRACK_ID"
    
    # Get file modification times if spec/plan exist
    local SPEC_ISO="null"
    local PLAN_ISO="null"
    local GEN_STATUS="initializing"
    
    if [[ -f "$TRACK_DIR/spec.md" ]]; then
      SPEC_MTIME=$(stat -f %m "$TRACK_DIR/spec.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/spec.md")
      SPEC_ISO="\"$(date -r $SPEC_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")\""
      GEN_STATUS="spec_done"
    fi
    
    if [[ -f "$TRACK_DIR/plan.md" ]]; then
      PLAN_MTIME=$(stat -f %m "$TRACK_DIR/plan.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/plan.md")
      PLAN_ISO="\"$(date -r $PLAN_MTIME -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || echo "$NOW")\""
      GEN_STATUS="plan_done"
    fi
    
    cat > "$TRACK_DIR/metadata.json.tmp.$$" << EOF
{
  "track_id": "$TRACK_ID",
  "type": "feature",
  "status": "new",
  "created_at": "$NOW",
  "updated_at": "$NOW",
  "description": "Auto-created by validation",
  "priority": "medium",
  "generation": {
    "status": "$GEN_STATUS",
    "specCreatedAt": $SPEC_ISO,
    "planCreatedAt": $PLAN_ISO,
    "rbCompletedAt": null
  },
  "beads": {
    "status": "pending",
    "epicId": null,
    "epics": [],
    "issues": [],
    "planTasks": {},
    "beadToTask": {},
    "crossTrackDeps": [],
    "reviewStatus": null,
    "reviewedAt": null
  },
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
}

# Usage:
auto_create_metadata "conductor/tracks/my-track_20251224"
```

## Update Generation Status

Update the generation section after spec/plan creation:

```bash
update_generation_status() {
  local TRACK_DIR="$1"
  local NEW_STATUS="$2"
  local FIELD="$3"  # specCreatedAt, planCreatedAt, or rbCompletedAt
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  if [[ -f "$TRACK_DIR/metadata.json" ]]; then
    jq --arg status "$NEW_STATUS" --arg field "$FIELD" --arg now "$NOW" \
       '.generation.status = $status | .generation[$field] = $now | .updated_at = $now' \
       "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp.$$"
    mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
  fi
}

# Usage:
update_generation_status "conductor/tracks/my-track_20251224" "spec_done" "specCreatedAt"
update_generation_status "conductor/tracks/my-track_20251224" "plan_done" "planCreatedAt"
update_generation_status "conductor/tracks/my-track_20251224" "complete" "rbCompletedAt"
```

## Update Beads Status

Update the beads section during filing:

```bash
update_beads_status() {
  local TRACK_DIR="$1"
  local NEW_STATUS="$2"
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  if [[ -f "$TRACK_DIR/metadata.json" ]]; then
    jq --arg status "$NEW_STATUS" --arg now "$NOW" \
       '.beads.status = $status | .updated_at = $now' \
       "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp.$$"
    mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
  fi
}

# Usage:
update_beads_status "conductor/tracks/my-track_20251224" "in_progress"
update_beads_status "conductor/tracks/my-track_20251224" "complete"
```

## Add Bead to planTasks Mapping

Add a bead ID to the planTasks mapping (bidirectional):

```bash
add_plan_task_mapping() {
  local TRACK_DIR="$1"
  local PLAN_TASK_ID="$2"
  local BEAD_ID="$3"
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  if [[ -f "$TRACK_DIR/metadata.json" ]]; then
    jq --arg taskId "$PLAN_TASK_ID" --arg beadId "$BEAD_ID" --arg now "$NOW" \
       '.beads.planTasks[$taskId] = $beadId | .beads.beadToTask[$beadId] = $taskId | .updated_at = $now' \
       "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp.$$"
    mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
  fi
}

# Usage:
add_plan_task_mapping "conductor/tracks/my-track_20251224" "1.1" "bd-42"
```

## Update Track Status

Update the top-level track status:

```bash
update_track_status() {
  local TRACK_DIR="$1"
  local NEW_STATUS="$2"
  local NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
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

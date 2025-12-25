# /conductor-migrate-beads Command

**Purpose:** Migrate existing tracks to use Beads integration. Scans tracks without beads, analyzes plans, and creates/links beads.

---

## Usage

```bash
/conductor-migrate-beads [track-id]
```

**Arguments:**
- `track-id` (optional): Specific track to migrate. If omitted, scans all tracks.

**Flags:**
- `--dry-run`: Show what would be done without making changes
- `--link-existing`: Link to existing beads by title match (vs creating new)
- `--force`: Overwrite existing `.fb-progress.json`

---

## When to Use

- Track was created before beads integration
- Track's beads were deleted/corrupted
- Importing tracks from another project
- `/conductor-status` shows `missing_bead` discrepancies

---

## Workflow Steps

### Phase 1: Scan

Find tracks without beads integration:

```bash
scan_tracks() {
  for track_dir in conductor/tracks/*/; do
    TRACK_ID=$(basename "$track_dir")
    FB_PROGRESS="${track_dir}/.fb-progress.json"
    PLAN="${track_dir}/plan.md"
    
    # Skip if no plan
    [[ -f "$PLAN" ]] || continue
    
    # Check for existing beads
    if [[ ! -f "$FB_PROGRESS" ]]; then
      echo "{\"trackId\": \"$TRACK_ID\", \"status\": \"no_integration\"}"
    elif [[ $(jq '.planTasks | length' "$FB_PROGRESS") -eq 0 ]]; then
      echo "{\"trackId\": \"$TRACK_ID\", \"status\": \"empty_mapping\"}"
    fi
  done | jq -s '.'
}
```

**Output:**

```
Scanning tracks for beads migration...

Tracks needing migration:
  - auth_20251220: no beads integration
  - api_20251218: empty planTasks mapping

Tracks with beads: 3
Tracks without beads: 2
```

---

### Phase 2: Analyze

For each track, parse plan.md to identify tasks:

```bash
analyze_track() {
  local TRACK_ID="$1"
  local PLAN="conductor/tracks/${TRACK_ID}/plan.md"
  
  # Extract tasks
  TASKS=$(grep -E '^\s*- \[([ x~])\] [0-9]+\.[0-9]+' "$PLAN" | while read -r line; do
    STATUS=$(echo "$line" | sed -E 's/.*\[([ x~])\].*/\1/')
    TASK_ID=$(echo "$line" | sed -E 's/.*- \[.\] ([0-9.]+):.*/\1/')
    TITLE=$(echo "$line" | sed -E 's/.*- \[.\] [0-9.]+: (.*)/\1/')
    
    echo "{\"id\": \"$TASK_ID\", \"title\": \"$TITLE\", \"status\": \"$STATUS\"}"
  done | jq -s '.')
  
  TASK_COUNT=$(echo "$TASKS" | jq 'length')
  
  echo "{\"trackId\": \"$TRACK_ID\", \"taskCount\": $TASK_COUNT, \"tasks\": $TASKS}"
}
```

**Output:**

```
Analyzing: auth_20251220

Plan structure:
  - 3 phases
  - 12 tasks total
  - 8 completed, 2 in-progress, 2 pending

Tasks to create:
  1.1.1: Create auth middleware
  1.1.2: Add JWT validation
  ...
```

---

### Phase 3: Confirm

Present migration plan to user:

```
━━━ MIGRATION PLAN: auth_20251220 ━━━

Actions:
  [C] Create new beads (12 issues + 1 epic)
  [L] Link to existing beads (by title match)
  [S] Skip this track

Current beads matching titles:
  - "Create auth middleware" → my-workflow:3-abc1 (closed)
  - "Add JWT validation" → my-workflow:3-xyz2 (open)

Choose action [C/L/S]:
```

### Link vs Create Decision

| Scenario | Recommended Action |
|----------|-------------------|
| No existing beads with matching titles | Create new |
| Existing beads match titles exactly | Link existing |
| Partial matches | Prompt user for each |
| `--link-existing` flag | Auto-link matches |

---

### Phase 4: Execute

#### Create New Beads

```bash
execute_create() {
  local TRACK_ID="$1"
  local ANALYSIS="$2"
  
  # Create epic
  EPIC_TITLE="Epic: $(grep -m1 '^# ' "conductor/tracks/${TRACK_ID}/plan.md" | sed 's/^# //')"
  EPIC_ID=$(bd create "$EPIC_TITLE" -t epic -p 0 --json | jq -r '.id')
  
  declare -A PLAN_TASKS
  
  # Create issues
  echo "$ANALYSIS" | jq -c '.tasks[]' | while read -r task; do
    TASK_ID=$(echo "$task" | jq -r '.id')
    TITLE=$(echo "$task" | jq -r '.title')
    STATUS=$(echo "$task" | jq -r '.status')
    
    # Determine priority from task ID
    PHASE=$(echo "$TASK_ID" | cut -d. -f1)
    case "$PHASE" in
      1|2) PRIORITY=0 ;;
      3) PRIORITY=1 ;;
      *) PRIORITY=2 ;;
    esac
    
    # Create issue
    BEAD_ID=$(bd create "$TITLE" -t task -p $PRIORITY --json | jq -r '.id')
    bd dep add "$BEAD_ID" "$EPIC_ID"
    
    # If already completed in plan, close bead
    if [[ "$STATUS" == "x" ]]; then
      bd close "$BEAD_ID" --reason completed
    elif [[ "$STATUS" == "~" ]]; then
      bd update "$BEAD_ID" --status in_progress
    fi
    
    PLAN_TASKS["$TASK_ID"]="$BEAD_ID"
    echo "Created: $TASK_ID → $BEAD_ID"
  done
  
  # Output mapping
  echo "${!PLAN_TASKS[@]}" | tr ' ' '\n' | while read -r k; do
    echo "\"$k\": \"${PLAN_TASKS[$k]}\""
  done | jq -s 'from_entries'
}
```

#### Link Existing Beads

```bash
execute_link() {
  local TRACK_ID="$1"
  local ANALYSIS="$2"
  
  declare -A PLAN_TASKS
  
  echo "$ANALYSIS" | jq -c '.tasks[]' | while read -r task; do
    TASK_ID=$(echo "$task" | jq -r '.id')
    TITLE=$(echo "$task" | jq -r '.title')
    
    # Find matching bead by title
    MATCH=$(bd list --json | jq -r --arg title "$TITLE" \
      '.[] | select(.title == $title or (.title | test($title; "i"))) | .id' | head -1)
    
    if [[ -n "$MATCH" ]]; then
      PLAN_TASKS["$TASK_ID"]="$MATCH"
      echo "Linked: $TASK_ID → $MATCH"
    else
      echo "WARN: No match for '$TITLE' - will create new"
      # Fall back to create
      BEAD_ID=$(bd create "$TITLE" -t task -p 2 --json | jq -r '.id')
      PLAN_TASKS["$TASK_ID"]="$BEAD_ID"
      echo "Created: $TASK_ID → $BEAD_ID"
    fi
  done
}
```

---

### Phase 5: Verify

Validate the migration:

```bash
verify_migration() {
  local TRACK_ID="$1"
  local FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  
  ERRORS=()
  
  # Check .fb-progress.json exists
  if [[ ! -f "$FB_PROGRESS" ]]; then
    ERRORS+=("Missing .fb-progress.json")
    return 1
  fi
  
  # Check planTasks mapping
  MAPPING_COUNT=$(jq '.planTasks | length' "$FB_PROGRESS")
  if [[ "$MAPPING_COUNT" -eq 0 ]]; then
    ERRORS+=("Empty planTasks mapping")
  fi
  
  # Verify each bead exists
  jq -r '.planTasks | to_entries[] | .value' "$FB_PROGRESS" | while read -r bead_id; do
    if ! bd show "$bead_id" --json >/dev/null 2>&1; then
      ERRORS+=("Bead not found: $bead_id")
    fi
  done
  
  if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo "Verification FAILED:"
    printf '  - %s\n' "${ERRORS[@]}"
    return 1
  fi
  
  echo "Verification PASSED"
  return 0
}
```

---

## Update .fb-progress.json

```bash
update_fb_progress() {
  local TRACK_ID="$1"
  local EPIC_ID="$2"
  local PLAN_TASKS="$3"  # JSON object
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  
  cat > "$FB_PROGRESS" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "complete",
  "migratedAt": "$NOW",
  "migrationType": "conductor-migrate-beads",
  "epics": ["$EPIC_ID"],
  "issues": $(echo "$PLAN_TASKS" | jq '[.[]]'),
  "planTasks": $PLAN_TASKS,
  "beadToTask": $(echo "$PLAN_TASKS" | jq 'to_entries | map({key: .value, value: .key}) | from_entries'),
  "lastVerified": "$NOW"
}
EOF
}
```

---

## Output Format

```
━━━ MIGRATION COMPLETE ━━━

Track: auth_20251220
Mode: Create new beads

Results:
  Epic created: my-workflow:3-epic1
  Issues created: 12
  Issues linked: 0
  
  Completed (closed): 8
  In-progress: 2
  Open: 2

Mapping saved to:
  conductor/tracks/auth_20251220/.fb-progress.json

Next: `/conductor-implement auth_20251220` to continue work
```

---

## Dry Run Mode

With `--dry-run`, show what would happen without changes:

```
━━━ DRY RUN: auth_20251220 ━━━

Would create:
  - 1 epic: "Epic: User Authentication"
  - 12 issues from plan.md
  
Would link:
  - 0 existing beads (no matches found)

Would update:
  - conductor/tracks/auth_20251220/.fb-progress.json

No changes made. Remove --dry-run to execute.
```

---

## Error Handling

| Error | Action |
|-------|--------|
| Track not found | Error with available tracks |
| plan.md missing | Skip track, warn user |
| bd create fails | Retry 3x, then HALT |
| Partial migration | Save progress, allow resume |
| .fb-progress.json exists | Error unless --force |

---

## References

- [Track Init Beads](../workflows/conductor/track-init-beads.md) - New track beads
- [Status Sync Beads](../workflows/conductor/status-sync-beads.md) - Discrepancy detection
- [Beads Integration](../skills/conductor/references/beads-integration.md) - Integration points

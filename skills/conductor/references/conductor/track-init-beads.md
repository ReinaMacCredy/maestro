# Track Init Beads Workflow

**Purpose:** Convert plan.md tasks to Beads issues during track initialization. Creates epic, issues, wires dependencies, and updates planTasks mapping.

---

## Overview

This workflow runs during `/conductor-newtrack` after spec and plan are confirmed:

1. **Parse** plan.md to extract tasks
2. **Validate** task structure
3. **Prompt** R/S/M if existing beads or malformed structure
4. **Create** epic from track title
5. **Create** issues from tasks
6. **Wire** dependencies between issues
7. **Update** `.fb-progress.json` with planTasks mapping

---

## Prerequisites

- Preflight completed (bd available)
- `plan.md` exists and is confirmed
- Track directory exists: `conductor/tracks/<track-id>/`

---

## Flow Diagram

```text
┌────────────────────────────────────────────────────────────────┐
│                    TRACK INIT BEADS FLOW                        │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   plan.md                                                       │
│       │                                                         │
│       ▼                                                         │
│   ┌─────────────────┐                                           │
│   │  Parse Tasks    │ ─── Extract phases, tasks, sub-tasks      │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐     ┌──────────────────────────────┐     │
│   │  Validate       │ ──► │  R/S/M Prompt (if issues)    │     │
│   │  Structure      │     │  [R]eformat / [S]kip / [M]anual    │
│   └────────┬────────┘     └──────────────────────────────┘     │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  Check Existing │ ─── bd list --json for track              │
│   │  Beads          │                                           │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐     ┌──────────────────────────────┐     │
│   │  Existing Found │ ──► │  R/S/M Prompt                │     │
│   │  ?              │     │  [R]eplace / [S]kip / [M]erge      │
│   └────────┬────────┘     └──────────────────────────────┘     │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  Create Epic    │ ─── bd create "<title>" -t epic           │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  Create Issues  │ ─── bd create "<task>" -t task -p <n>     │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  Wire Deps      │ ─── bd dep add <issue> <epic|dep>         │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  Update Mapping │ ─── .fb-progress.json                     │
│   └─────────────────┘                                           │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## Step 1: Parse plan.md

Extract tasks from plan structure.

### Expected Plan Structure

```markdown
## Phase 1: Foundation

### Epic 1.1: State Validation

- [ ] 1.1.1: Create beads-facade.md
  - Define facade contract
  - **Est:** 140-180 lines

- [ ] 1.1.2: Create beads-integration.md
  - Document integration points
```

### Parsing Algorithm

```bash
parse_plan() {
  local PLAN_PATH="$1"
  
  # Extract tasks matching pattern: "- [x/ /~] X.Y.Z: Title" (colon optional, flexible whitespace)
  # Captures open ([ ]), done ([x]), and in-progress ([~]) tasks
  grep -E '^\s*- \[([ x~])\] [0-9]+\.[0-9]+(\.[0-9]+)?:?\s*' "$PLAN_PATH" | while read -r line; do
    # Extract checkbox state (space=open, x=done, ~=in-progress)
    CHECKBOX=$(echo "$line" | sed -E 's/.*- \[(.)\] .*/\1/')
    
    # Map checkbox to bead status
    case "$CHECKBOX" in
      " ") STATUS="open" ;;
      "x") STATUS="closed" ;;
      "~") STATUS="in_progress" ;;
      *)   STATUS="open" ;;
    esac
    
    # Extract task ID (e.g., "1.1.1")
    TASK_ID=$(echo "$line" | sed -E 's/.*- \[.\] ([0-9]+\.[0-9]+(\.[0-9]+)?):?\s*.*/\1/')
    
    # Extract title
    TITLE=$(echo "$line" | sed -E 's/.*- \[.\] [0-9]+\.[0-9]+(\.[0-9]+)?:?\s*(.*)/\2/')
    
    # Output as JSON
    echo "{\"id\": \"$TASK_ID\", \"title\": \"$TITLE\", \"status\": \"$STATUS\"}"
  done
}
```

### Output Format

```json
[
  {"id": "1.1.1", "title": "Create beads-facade.md", "priority": 0},
  {"id": "1.1.2", "title": "Create beads-integration.md", "priority": 0},
  {"id": "1.2.1", "title": "Create preflight-beads.md", "priority": 0, "depends": ["1.1.1", "1.1.2"]}
]
```

---

## Step 2: Validate Structure

Check for required elements.

### Validation Checks

| Check | Required | Error Message |
|-------|----------|---------------|
| Task IDs unique | Yes | "Duplicate task ID: X.Y.Z" |
| Task IDs sequential | No | Warning only |
| Dependency targets exist | Yes | "Dependency target not found: X.Y.Z" |
| At least one task | Yes | "Plan has no tasks" |

### Validation Script

```bash
validate_plan_structure() {
  local TASKS_JSON="$1"
  local ERRORS=()
  
  # Check for duplicates
  DUPLICATES=$(echo "$TASKS_JSON" | jq -r '.[].id' | sort | uniq -d)
  if [[ -n "$DUPLICATES" ]]; then
    ERRORS+=("Duplicate task IDs: $DUPLICATES")
  fi
  
  # Check for empty plan
  TASK_COUNT=$(echo "$TASKS_JSON" | jq 'length')
  if [[ "$TASK_COUNT" -eq 0 ]]; then
    ERRORS+=("Plan has no tasks")
  fi
  
  # Check dependency targets
  ALL_IDS=$(echo "$TASKS_JSON" | jq -r '.[].id')
  echo "$TASKS_JSON" | jq -c '.[] | select(.depends) | .depends[]' | while read -r dep; do
    dep=$(echo "$dep" | tr -d '"')
    if ! echo "$ALL_IDS" | grep -q "^${dep}$"; then
      ERRORS+=("Dependency target not found: $dep")
    fi
  done
  
  if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo "VALIDATION_FAILED"
    printf '%s\n' "${ERRORS[@]}"
    return 1
  fi
  
  echo "VALIDATION_OK"
  return 0
}
```

---

## Step 3: R/S/M Prompt (Structure Issues)

When validation fails, prompt user.

### Prompt Display

```
⚠️ Plan structure issues detected:
- Duplicate task IDs: 1.1.1
- Dependency target not found: 1.0.1

Options:
  [R]eformat - Auto-fix issues and continue
  [S]kip - Skip beads filing (plan-only mode)
  [M]anual - Abort for manual fix

Choice [R/S/M]:
```

### Behavior by Choice

| Choice | Action |
|--------|--------|
| **R**eformat | Auto-renumber duplicates, remove invalid deps |
| **S**kip | Set `skip_beads=true`, continue to handoff |
| **M**anual | HALT with error details |

### `--strict` Flag (CI Mode)

When `--strict` is set:
- No interactive prompt
- Fail immediately on validation errors
- Exit with code 1

```bash
if [[ "$STRICT_MODE" == "true" ]]; then
  echo "ERROR: --strict mode, validation failed"
  exit 1
fi
```

---

## Step 4: Check Existing Beads

Detect if beads already exist for this track.

```bash
check_existing_beads() {
  local TRACK_ID="$1"
  
  # Check .fb-progress.json
  FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  if [[ -f "$FB_PROGRESS" ]]; then
    EXISTING_EPICS=$(jq -r '.epics // [] | length' "$FB_PROGRESS")
    EXISTING_ISSUES=$(jq -r '.issues // [] | length' "$FB_PROGRESS")
    
    if [[ "$EXISTING_EPICS" -gt 0 || "$EXISTING_ISSUES" -gt 0 ]]; then
      echo "EXISTING_BEADS"
      echo "Epics: $EXISTING_EPICS, Issues: $EXISTING_ISSUES"
      return 0
    fi
  fi
  
  echo "NO_EXISTING"
  return 1
}
```

---

## Step 5: R/S/M Prompt (Existing Beads)

When existing beads are found, prompt user.

### Prompt Display

```
Existing beads found for track:
- 1 epic, 12 issues
- Last updated: 2025-12-24

Options:
  [R]eplace - Delete existing, create fresh
  [S]kip - Keep existing, skip filing
  [M]erge - Link new tasks to existing beads

Choice [R/S/M]:
```

### Behavior by Choice

| Choice | Action |
|--------|--------|
| **R**eplace | Close existing beads, create new epic + issues |
| **S**kip | Keep existing, update mapping only |
| **M**erge | Match tasks by title, create only new ones |

### Merge Algorithm

```bash
merge_with_existing() {
  local TASKS_JSON="$1"
  local FB_PROGRESS="$2"
  
  # Load existing planTasks mapping
  EXISTING_MAPPING=$(jq '.planTasks // {}' "$FB_PROGRESS")
  
  echo "$TASKS_JSON" | jq -c '.[]' | while read -r task; do
    TASK_ID=$(echo "$task" | jq -r '.id')
    TITLE=$(echo "$task" | jq -r '.title')
    
    # Check if task already mapped
    EXISTING_BEAD=$(echo "$EXISTING_MAPPING" | jq -r --arg id "$TASK_ID" '.[$id] // empty')
    
    if [[ -n "$EXISTING_BEAD" ]]; then
      echo "SKIP: $TASK_ID → $EXISTING_BEAD (already mapped)"
    else
      # Try to find by title match
      MATCH=$(bd list --json | jq -r --arg title "$TITLE" '.[] | select(.title == $title) | .id' | head -1)
      
      if [[ -n "$MATCH" ]]; then
        echo "LINK: $TASK_ID → $MATCH (title match)"
      else
        echo "CREATE: $TASK_ID (new)"
      fi
    fi
  done
}
```

---

## Step 6: Create Epic

Create the track's epic bead.

```bash
create_epic() {
  local TRACK_ID="$1"
  local PLAN_PATH="$2"
  
  # Extract epic title from track description or plan header
  EPIC_TITLE="Epic: $(grep -m1 '^# ' "$PLAN_PATH" | sed 's/^# //')"
  
  # Create epic
  RESULT=$(bd create "$EPIC_TITLE" -t epic -p 0 --json 2>&1)
  EXIT_CODE=$?
  
  if [[ $EXIT_CODE -ne 0 ]]; then
    echo "ERROR: Failed to create epic"
    echo "$RESULT"
    return 1
  fi
  
  EPIC_ID=$(echo "$RESULT" | jq -r '.id')
  echo "$EPIC_ID"
}
```

---

## Step 7: Create Issues

Create issue beads for each task.

### Priority Mapping

| Plan Phase | Priority |
|------------|----------|
| Phase 1 (Foundation) | P0 (0) |
| Phase 2 (Core) | P0 (0) |
| Phase 3 (Extensions) | P1 (1) |
| Phase 4 (Polish) | P2 (2) |

### Issue Creation

```bash
create_issues() {
  local TASKS_JSON="$1"
  local EPIC_ID="$2"
  
  declare -A TASK_TO_BEAD
  
  echo "$TASKS_JSON" | jq -c '.[]' | while read -r task; do
    TASK_ID=$(echo "$task" | jq -r '.id')
    TITLE=$(echo "$task" | jq -r '.title')
    PRIORITY=$(echo "$task" | jq -r '.priority // 2')
    
    # Create issue
    RESULT=$(bd create "$TITLE" -t task -p "$PRIORITY" --json 2>&1)
    if [[ $? -eq 0 ]]; then
      BEAD_ID=$(echo "$RESULT" | jq -r '.id')
      TASK_TO_BEAD["$TASK_ID"]="$BEAD_ID"
      
      # Link to epic
      bd dep add "$BEAD_ID" "$EPIC_ID"
      
      echo "Created: $TASK_ID → $BEAD_ID"
    else
      echo "WARN: Failed to create issue for $TASK_ID"
    fi
  done
  
  # Output mapping as JSON
  echo "${TASK_TO_BEAD[@]}" | jq -R 'split(" ") | to_entries | map({key: .key, value: .value}) | from_entries'
}
```

---

## Step 8: Wire Dependencies

Link issues based on task dependencies.

```bash
wire_dependencies() {
  local TASKS_JSON="$1"
  local PLAN_TASKS_MAPPING="$2"  # JSON: {"1.1.1": "bd-42", ...}
  
  echo "$TASKS_JSON" | jq -c '.[] | select(.depends)' | while read -r task; do
    TASK_ID=$(echo "$task" | jq -r '.id')
    BEAD_ID=$(echo "$PLAN_TASKS_MAPPING" | jq -r --arg id "$TASK_ID" '.[$id]')
    
    echo "$task" | jq -r '.depends[]' | while read -r dep_task; do
      DEP_BEAD=$(echo "$PLAN_TASKS_MAPPING" | jq -r --arg id "$dep_task" '.[$id]')
      
      if [[ -n "$DEP_BEAD" && "$DEP_BEAD" != "null" ]]; then
        bd dep add "$BEAD_ID" "$DEP_BEAD"
        echo "Linked: $BEAD_ID depends on $DEP_BEAD"
      fi
    done
  done
}
```

---

## Step 9: Update .fb-progress.json

Update the progress file with mapping.

```bash
update_fb_progress() {
  local TRACK_ID="$1"
  local EPIC_ID="$2"
  local PLAN_TASKS_MAPPING="$3"
  local THREAD_ID="$4"
  
  FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Create or update .fb-progress.json
  cat > "$FB_PROGRESS" << EOF
{
  "trackId": "$TRACK_ID",
  "status": "complete",
  "startedAt": "$NOW",
  "threadId": "$THREAD_ID",
  "resumeFrom": null,
  "epics": ["$EPIC_ID"],
  "issues": $(echo "$PLAN_TASKS_MAPPING" | jq '[.[]]'),
  "planTasks": $PLAN_TASKS_MAPPING,
  "beadToTask": $(echo "$PLAN_TASKS_MAPPING" | jq 'to_entries | map({key: .value, value: .key}) | from_entries'),
  "crossTrackDeps": [],
  "lastError": null,
  "lastVerified": "$NOW"
}
EOF
  
  echo "Updated: $FB_PROGRESS"
}
```

---

## Complete Flow Example

```bash
#!/bin/bash
# track-init-beads.sh

TRACK_ID="$1"
PLAN_PATH="conductor/tracks/${TRACK_ID}/plan.md"
THREAD_ID="${THREAD_ID:-unknown}"
STRICT_MODE="${STRICT_MODE:-false}"

# Step 1: Parse plan
TASKS=$(parse_plan "$PLAN_PATH")

# Step 2: Validate
VALIDATION=$(validate_plan_structure "$TASKS")
if [[ "$VALIDATION" == "VALIDATION_FAILED"* ]]; then
  if [[ "$STRICT_MODE" == "true" ]]; then
    echo "ERROR: Validation failed in strict mode"
    exit 1
  fi
  # Prompt R/S/M
  # ... handle response
fi

# Step 4: Check existing
EXISTING=$(check_existing_beads "$TRACK_ID")
if [[ "$EXISTING" == "EXISTING_BEADS"* ]]; then
  # Prompt R/S/M
  # ... handle response
fi

# Step 6: Create epic
EPIC_ID=$(create_epic "$TRACK_ID" "$PLAN_PATH")

# Step 7: Create issues
PLAN_TASKS_MAPPING=$(create_issues "$TASKS" "$EPIC_ID")

# Step 8: Wire dependencies
wire_dependencies "$TASKS" "$PLAN_TASKS_MAPPING"

# Step 9: Update mapping
update_fb_progress "$TRACK_ID" "$EPIC_ID" "$PLAN_TASKS_MAPPING" "$THREAD_ID"

echo "Track init complete: $TRACK_ID"
echo "Epic: $EPIC_ID"
echo "Issues: $(echo "$PLAN_TASKS_MAPPING" | jq 'length')"
```

---

## Error Handling

| Error | Action |
|-------|--------|
| Plan not found | HALT with error |
| Validation failed + strict | HALT with exit 1 |
| bd create fails | Retry 3x, then HALT |
| Dependency wiring fails | Log warning, continue |
| .fb-progress.json write fails | HALT with error |

---

## Output Summary

On successful completion:

```
Track Init Beads: Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Track: beads-integration_20251225
Epic: my-workflow:3-abc1
Issues: 12 created
Dependencies: 8 wired

Ready tasks: 4
First ready: my-workflow:3-def2 - Create beads-facade.md
```

---

## References

- [Beads Facade](../beads-facade.md) - API contract
- [Beads Integration](../beads-integration.md) - All 13 points
- [Preflight Workflow](preflight-beads.md) - Session initialization
- [Session Workflow](beads-session.md) - Claim/close/sync

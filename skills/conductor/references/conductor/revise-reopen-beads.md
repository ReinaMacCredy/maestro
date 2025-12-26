# Revise Reopen Beads Workflow

**Purpose:** Handle bead lifecycle when specs/plans are revised. Reopens affected beads or creates new ones with lineage preservation.

---

## Overview

When `/conductor-revise` modifies a spec or plan, this workflow:

1. Identifies affected beads
2. Determines action (reopen vs create new)
3. Preserves history and lineage
4. Updates planTasks mapping

---

## Prerequisites

- Track has beads integration (`.fb-progress.json` exists)
- Spec or plan changes identified
- Preflight completed

---

## When to Trigger

This workflow triggers when:

1. `/conductor-revise` modifies spec.md or plan.md
2. Tasks are added, removed, or significantly changed
3. User explicitly requests bead reopening

---

## Flow Diagram

```text
┌────────────────────────────────────────────────────────────────┐
│                    REVISE REOPEN FLOW                           │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Spec/Plan Changes                                             │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────────────┐                                           │
│   │  Identify       │ ─── Compare old vs new plan               │
│   │  Affected Tasks │                                           │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  For Each Task  │                                           │
│   └────────┬────────┘                                           │
│            │                                                    │
│     ┌──────┴──────┐                                             │
│     │             │                                             │
│     ▼             ▼                                             │
│   ┌─────────┐   ┌─────────┐                                     │
│   │ Bead    │   │ Bead    │                                     │
│   │ Exists  │   │ Deleted │                                     │
│   └────┬────┘   └────┬────┘                                     │
│        │             │                                          │
│        ▼             ▼                                          │
│   ┌─────────┐   ┌─────────┐                                     │
│   │ REOPEN  │   │ CREATE  │                                     │
│   │ Bead    │   │ New w/  │                                     │
│   │         │   │ Lineage │                                     │
│   └────┬────┘   └────┬────┘                                     │
│        │             │                                          │
│        └──────┬──────┘                                          │
│               │                                                 │
│               ▼                                                 │
│   ┌─────────────────┐                                           │
│   │  Update Mapping │                                           │
│   └─────────────────┘                                           │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## Step 1: Identify Affected Tasks

Compare current plan with beads mapping to find:
- **Modified tasks:** Title or scope changed
- **New tasks:** In plan but no bead
- **Removed tasks:** Bead exists but not in plan

```bash
identify_affected_tasks() {
  local TRACK_ID="$1"
  local PLAN_PATH="conductor/tracks/${TRACK_ID}/plan.md"
  local FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  
  # Get current plan tasks
  PLAN_TASKS=$(grep -E '^\s*- \[ \] [0-9]+\.[0-9]+' "$PLAN_PATH" | \
               sed -E 's/.*- \[ \] ([0-9.]+): (.*)/\1|\2/')
  
  # Get existing mapping
  EXISTING_MAPPING=$(jq -r '.planTasks // {}' "$FB_PROGRESS")
  
  AFFECTED=()
  
  # Check each plan task
  while IFS='|' read -r taskId title; do
    BEAD_ID=$(echo "$EXISTING_MAPPING" | jq -r --arg id "$taskId" '.[$id] // empty')
    
    if [[ -z "$BEAD_ID" ]]; then
      AFFECTED+=("{\"taskId\": \"$taskId\", \"action\": \"create\", \"title\": \"$title\"}")
    else
      # Check if bead still exists
      BEAD_EXISTS=$(bd show "$BEAD_ID" --json 2>/dev/null | jq 'length > 0')
      
      if [[ "$BEAD_EXISTS" == "true" ]]; then
        BEAD_STATUS=$(bd show "$BEAD_ID" --json | jq -r '.[0].status')
        
        if [[ "$BEAD_STATUS" == "closed" ]]; then
          AFFECTED+=("{\"taskId\": \"$taskId\", \"action\": \"reopen\", \"beadId\": \"$BEAD_ID\"}")
        fi
      else
        AFFECTED+=("{\"taskId\": \"$taskId\", \"action\": \"create_with_lineage\", \"originalBeadId\": \"$BEAD_ID\", \"title\": \"$title\"}")
      fi
    fi
  done <<< "$PLAN_TASKS"
  
  printf '%s\n' "${AFFECTED[@]}" | jq -s '.'
}
```

---

## Step 2: Reopen Existing Bead

When the bead still exists but was closed:

```bash
reopen_bead() {
  local BEAD_ID="$1"
  local REASON="$2"  # e.g., "spec revision"
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Get current notes
  CURRENT_NOTES=$(bd show "$BEAD_ID" --json | jq -r '.[0].notes // ""')
  
  # Append reopen note
  NEW_NOTES="${CURRENT_NOTES}

---
REOPENED: $NOW
REASON: $REASON"
  
  # Update status and notes
  bd update "$BEAD_ID" --status open --notes "$NEW_NOTES"
  
  echo "Reopened: $BEAD_ID"
}
```

---

## Step 3: Create New Bead with Lineage

When the original bead was deleted (cleaned up):

```bash
create_with_lineage() {
  local TASK_ID="$1"
  local TITLE="$2"
  local ORIGINAL_BEAD_ID="$3"
  local REASON="$4"
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Create new bead with lineage metadata
  DESCRIPTION="Rework of original task.

---
LINEAGE:
- Original Bead: $ORIGINAL_BEAD_ID (deleted/cleaned up)
- Reopened At: $NOW
- Reopen Reason: $REASON"
  
  NEW_BEAD=$(bd create "Rework: $TITLE" -t task -p 0 --description "$DESCRIPTION" --json)
  NEW_BEAD_ID=$(echo "$NEW_BEAD" | jq -r '.id')
  
  echo "$NEW_BEAD_ID"
}
```

### Lineage Schema

```json
{
  "title": "Rework: Original task title",
  "notes": "Reopened from cleaned-up bd-42. Original closed 2025-12-20.",
  "metadata": {
    "originalBeadId": "bd-42",
    "reopenedAt": "2025-12-25T10:00:00Z",
    "reopenReason": "spec revision"
  }
}
```

---

## Step 4: Update planTasks Mapping

```bash
update_mapping() {
  local TRACK_ID="$1"
  local TASK_ID="$2"
  local NEW_BEAD_ID="$3"
  
  FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  
  # Update planTasks
  jq --arg task "$TASK_ID" --arg bead "$NEW_BEAD_ID" \
    '.planTasks[$task] = $bead | .beadToTask[$bead] = $task' \
    "$FB_PROGRESS" > "$FB_PROGRESS.tmp.$$"
  mv "$FB_PROGRESS.tmp.$$" "$FB_PROGRESS"
  
  echo "Updated mapping: $TASK_ID → $NEW_BEAD_ID"
}
```

---

## History Preservation Rules

| Scenario | Action | History |
|----------|--------|---------|
| Bead exists, closed | Reopen | Append reopen note to existing notes |
| Bead deleted (cleanup) | Create new | Include lineage in description |
| Bead open | No action | Already available |
| Task scope changed | Update bead | Append scope change note |

### Notes Format for Reopened Beads

```text
ORIGINAL COMPLETION:
COMPLETED: Implemented feature X
KEY DECISION: Used approach Y

---
REOPENED: 2025-12-25T10:00:00Z
REASON: spec revision - requirements changed
SCOPE: Additional validation needed
```

---

## Complete Workflow

```bash
revise_reopen_workflow() {
  local TRACK_ID="$1"
  local REOPEN_REASON="$2"  # e.g., "spec revision"
  
  echo "Identifying affected tasks..."
  AFFECTED=$(identify_affected_tasks "$TRACK_ID")
  
  AFFECTED_COUNT=$(echo "$AFFECTED" | jq 'length')
  if [[ "$AFFECTED_COUNT" -eq 0 ]]; then
    echo "No beads need updating"
    return 0
  fi
  
  echo "Found $AFFECTED_COUNT affected beads"
  echo ""
  
  echo "$AFFECTED" | jq -c '.[]' | while read -r item; do
    TASK_ID=$(echo "$item" | jq -r '.taskId')
    ACTION=$(echo "$item" | jq -r '.action')
    
    case "$ACTION" in
      reopen)
        BEAD_ID=$(echo "$item" | jq -r '.beadId')
        reopen_bead "$BEAD_ID" "$REOPEN_REASON"
        ;;
      create)
        TITLE=$(echo "$item" | jq -r '.title')
        NEW_ID=$(bd create "$TITLE" -t task -p 0 --json | jq -r '.id')
        update_mapping "$TRACK_ID" "$TASK_ID" "$NEW_ID"
        echo "Created: $TASK_ID → $NEW_ID"
        ;;
      create_with_lineage)
        TITLE=$(echo "$item" | jq -r '.title')
        ORIGINAL_ID=$(echo "$item" | jq -r '.originalBeadId')
        NEW_ID=$(create_with_lineage "$TASK_ID" "$TITLE" "$ORIGINAL_ID" "$REOPEN_REASON")
        update_mapping "$TRACK_ID" "$TASK_ID" "$NEW_ID"
        echo "Created with lineage: $TASK_ID → $NEW_ID (was $ORIGINAL_ID)"
        ;;
    esac
  done
  
  echo ""
  echo "Revise complete. Beads updated: $AFFECTED_COUNT"
}
```

---

## Integration with /conductor-revise

```markdown
## After Spec/Plan Changes

1. User modifies spec.md or plan.md via `/conductor-revise`

2. Workflow detects changes:
   - New tasks added → create beads
   - Existing tasks modified → check if bead needs reopening
   - Tasks removed → close orphan beads (optional)

3. For each affected task:
   - If bead exists and closed → reopen with history
   - If bead deleted → create new with lineage
   - Update planTasks mapping

4. Report summary:
   - X beads reopened
   - Y new beads created
   - Z mappings updated
```

---

## Error Handling

| Error | Action |
|-------|--------|
| bd show fails | Skip bead, log warning |
| bd update fails | Retry 3x, persist pending |
| bd create fails | HALT (cannot continue) |
| Mapping update fails | Log error, continue |
| Original bead not found | Create with lineage |

---

## Output Format

```text
━━━ REVISE REOPEN: auth_20251225 ━━━

Reason: spec revision - added 2FA requirement

Affected Tasks:
  - 1.2.3: Reopened (my-workflow:3-abc1)
  - 2.1.1: Created new (my-workflow:3-xyz2)
  - 2.1.2: Created with lineage (was my-workflow:3-old3)

Summary:
  Reopened: 1
  Created: 2
  Mapping updated: 3
```

---

## References

- [Beads Integration](../../skills/conductor/references/beads-integration.md) - Point 13
- [Status Sync](status-sync-beads.md) - Discrepancy detection
- [Beads Facade](../../skills/conductor/references/beads-facade.md) - API contract

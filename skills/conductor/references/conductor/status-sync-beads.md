# Status Sync Beads Workflow

**Purpose:** Bidirectional synchronization between Conductor track state and Beads issue state. Detects discrepancies and suggests reconciliation.

---

## Overview

This workflow runs during `/conductor-status` to:

1. Query Conductor state (plan.md, metadata.json)
2. Query Beads state (bd list --json)
3. Compare and detect discrepancies
4. Report findings and suggest reconciliation

---

## Prerequisites

- Preflight completed
- Track exists with `.fb-progress.json` (planTasks mapping)

---

## Flow Diagram

```text
┌────────────────────────────────────────────────────────────────┐
│                    STATUS SYNC FLOW                             │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────────┐         ┌─────────────────┐              │
│   │  Conductor      │         │  Beads          │              │
│   │  State          │         │  State          │              │
│   │  - plan.md      │         │  - bd list      │              │
│   │  - metadata     │         │  - issue status │              │
│   └────────┬────────┘         └────────┬────────┘              │
│            │                           │                        │
│            └───────────┬───────────────┘                        │
│                        │                                        │
│                        ▼                                        │
│               ┌─────────────────┐                               │
│               │  COMPARE        │                               │
│               │  Find mismatches│                               │
│               └────────┬────────┘                               │
│                        │                                        │
│                        ▼                                        │
│               ┌─────────────────┐                               │
│               │  REPORT         │                               │
│               │  Show discrepancies                             │
│               └────────┬────────┘                               │
│                        │                                        │
│                        ▼                                        │
│               ┌─────────────────┐                               │
│               │  SUGGEST        │                               │
│               │  Reconciliation │                               │
│               └─────────────────┘                               │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## Step 1: Query Conductor State

```bash
query_conductor_state() {
  local TRACK_ID="$1"
  local TRACK_DIR="conductor/tracks/${TRACK_ID}"
  
  # Read plan.md task statuses
  PLAN_TASKS=$(grep -E '^\s*- \[([ x~])\]' "${TRACK_DIR}/plan.md" | while read -r line; do
    STATUS=$(echo "$line" | sed -E 's/.*\[([ x~])\].*/\1/')
    TASK_ID=$(echo "$line" | sed -E 's/.*- \[.\] ([0-9.]+):.*/\1/')
    
    case "$STATUS" in
      " ") STATUS="todo" ;;
      "~") STATUS="in_progress" ;;
      "x") STATUS="done" ;;
    esac
    
    echo "{\"taskId\": \"$TASK_ID\", \"conductorStatus\": \"$STATUS\"}"
  done | jq -s '.')
  
  # Read metadata
  METADATA=$(cat "${TRACK_DIR}/metadata.json" 2>/dev/null || echo '{}')
  TRACK_STATUS=$(echo "$METADATA" | jq -r '.status // "unknown"')
  
  echo "{\"trackStatus\": \"$TRACK_STATUS\", \"tasks\": $PLAN_TASKS}"
}
```

---

## Step 2: Query Beads State

```bash
query_beads_state() {
  local TRACK_ID="$1"
  local FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"
  
  # Get planTasks mapping
  if [[ ! -f "$FB_PROGRESS" ]]; then
    echo '{"error": "No beads integration found"}'
    return 1
  fi
  
  PLAN_TASKS=$(jq '.planTasks // {}' "$FB_PROGRESS")
  
  # Query each bead's status
  echo "$PLAN_TASKS" | jq -r 'to_entries | .[] | "\(.key) \(.value)"' | while read -r taskId beadId; do
    BEAD_STATUS=$(bd show "$beadId" --json 2>/dev/null | jq -r '.[0].status // "unknown"')
    echo "{\"taskId\": \"$taskId\", \"beadId\": \"$beadId\", \"beadStatus\": \"$BEAD_STATUS\"}"
  done | jq -s '.'
}
```

---

## Step 3: Compare States

```bash
compare_states() {
  local CONDUCTOR_STATE="$1"
  local BEADS_STATE="$2"
  
  DISCREPANCIES=()
  
  # For each task, compare statuses
  echo "$CONDUCTOR_STATE" | jq -c '.tasks[]' | while read -r conductor_task; do
    TASK_ID=$(echo "$conductor_task" | jq -r '.taskId')
    CONDUCTOR_STATUS=$(echo "$conductor_task" | jq -r '.conductorStatus')
    
    # Find matching bead
    BEAD_INFO=$(echo "$BEADS_STATE" | jq -r --arg id "$TASK_ID" '.[] | select(.taskId == $id)')
    
    if [[ -z "$BEAD_INFO" ]]; then
      echo "{\"type\": \"missing_bead\", \"taskId\": \"$TASK_ID\", \"conductorStatus\": \"$CONDUCTOR_STATUS\"}"
      continue
    fi
    
    BEAD_STATUS=$(echo "$BEAD_INFO" | jq -r '.beadStatus')
    BEAD_ID=$(echo "$BEAD_INFO" | jq -r '.beadId')
    
    # Map bead status to conductor status
    case "$BEAD_STATUS" in
      open) MAPPED_STATUS="todo" ;;
      in_progress) MAPPED_STATUS="in_progress" ;;
      closed) MAPPED_STATUS="done" ;;
      *) MAPPED_STATUS="unknown" ;;
    esac
    
    # Check for mismatch
    if [[ "$CONDUCTOR_STATUS" != "$MAPPED_STATUS" ]]; then
      echo "{\"type\": \"status_mismatch\", \"taskId\": \"$TASK_ID\", \"beadId\": \"$BEAD_ID\", \"conductorStatus\": \"$CONDUCTOR_STATUS\", \"beadStatus\": \"$BEAD_STATUS\"}"
    fi
  done | jq -s '.'
}
```

---

## Discrepancy Types

| Type | Description | Suggestion |
|------|-------------|------------|
| `status_mismatch` | Conductor and Beads disagree | Update the lagging source |
| `missing_bead` | Task in plan, no bead | Run `/conductor-migrate-beads` |
| `orphan_bead` | Bead exists, not in plan | Close bead or add to plan |
| `stale_progress` | Bead done, plan not updated | Mark `[x]` in plan.md |

---

## Step 4: Report Discrepancies

```bash
report_discrepancies() {
  local DISCREPANCIES="$1"
  local COUNT=$(echo "$DISCREPANCIES" | jq 'length')
  
  if [[ "$COUNT" -eq 0 ]]; then
    echo "✓ Conductor and Beads are in sync"
    return 0
  fi
  
  echo ""
  echo "⚠️ Status Sync: $COUNT discrepancies found"
  echo ""
  
  echo "$DISCREPANCIES" | jq -c '.[]' | while read -r disc; do
    TYPE=$(echo "$disc" | jq -r '.type')
    TASK_ID=$(echo "$disc" | jq -r '.taskId')
    
    case "$TYPE" in
      status_mismatch)
        CONDUCTOR=$(echo "$disc" | jq -r '.conductorStatus')
        BEAD=$(echo "$disc" | jq -r '.beadStatus')
        BEAD_ID=$(echo "$disc" | jq -r '.beadId')
        echo "  - Task $TASK_ID: plan=$CONDUCTOR, bead=$BEAD ($BEAD_ID)"
        ;;
      missing_bead)
        CONDUCTOR=$(echo "$disc" | jq -r '.conductorStatus')
        echo "  - Task $TASK_ID: in plan ($CONDUCTOR) but no bead"
        ;;
      orphan_bead)
        BEAD_ID=$(echo "$disc" | jq -r '.beadId')
        echo "  - Bead $BEAD_ID: exists but not in plan"
        ;;
    esac
  done
}
```

---

## Step 5: Suggest Reconciliation

```bash
suggest_reconciliation() {
  local DISCREPANCIES="$1"
  
  echo ""
  echo "Reconciliation Options:"
  echo ""
  
  HAS_MISMATCHES=$(echo "$DISCREPANCIES" | jq 'any(.type == "status_mismatch")')
  HAS_MISSING=$(echo "$DISCREPANCIES" | jq 'any(.type == "missing_bead")')
  HAS_ORPHANS=$(echo "$DISCREPANCIES" | jq 'any(.type == "orphan_bead")')
  
  if [[ "$HAS_MISMATCHES" == "true" ]]; then
    echo "  [B] Update Beads to match Conductor"
    echo "  [C] Update Conductor to match Beads"
  fi
  
  if [[ "$HAS_MISSING" == "true" ]]; then
    echo "  [M] Run /conductor-migrate-beads to create missing beads"
  fi
  
  if [[ "$HAS_ORPHANS" == "true" ]]; then
    echo "  [O] Close orphan beads"
  fi
  
  echo "  [S] Skip (no action)"
  echo ""
}
```

---

## Reconciliation Actions

### Update Beads to Match Conductor

```bash
reconcile_to_beads() {
  local DISCREPANCIES="$1"
  
  echo "$DISCREPANCIES" | jq -c '.[] | select(.type == "status_mismatch")' | while read -r disc; do
    BEAD_ID=$(echo "$disc" | jq -r '.beadId')
    CONDUCTOR_STATUS=$(echo "$disc" | jq -r '.conductorStatus')
    
    case "$CONDUCTOR_STATUS" in
      done)
        bd close "$BEAD_ID" --reason completed
        ;;
      in_progress)
        bd update "$BEAD_ID" --status in_progress
        ;;
      todo)
        bd update "$BEAD_ID" --status open
        ;;
    esac
  done
}
```

### Update Conductor to Match Beads

```bash
reconcile_to_conductor() {
  local DISCREPANCIES="$1"
  local PLAN_PATH="$2"
  
  echo "$DISCREPANCIES" | jq -c '.[] | select(.type == "status_mismatch")' | while read -r disc; do
    TASK_ID=$(echo "$disc" | jq -r '.taskId')
    BEAD_STATUS=$(echo "$disc" | jq -r '.beadStatus')
    
    case "$BEAD_STATUS" in
      closed)
        # Mark [x] in plan
        sed -i '' "s/\- \[ \] ${TASK_ID}:/- [x] ${TASK_ID}:/" "$PLAN_PATH"
        ;;
      in_progress)
        # Mark [~] in plan
        sed -i '' "s/\- \[ \] ${TASK_ID}:/- [~] ${TASK_ID}:/" "$PLAN_PATH"
        ;;
      open)
        # Mark [ ] in plan
        sed -i '' "s/\- \[x\] ${TASK_ID}:/- [ ] ${TASK_ID}:/" "$PLAN_PATH"
        ;;
    esac
  done
}
```

---

## Output Format

```text
━━━ STATUS SYNC: auth_20251225 ━━━

Conductor State:
  Track: in_progress
  Tasks: 12 total (8 done, 2 in_progress, 2 todo)

Beads State:
  Epic: my-workflow:3-abc1
  Issues: 12 (8 closed, 2 in_progress, 2 open)

⚠️ 2 discrepancies found:

  - Task 1.2.3: plan=done, bead=in_progress (my-workflow:3-xyz)
  - Task 2.1.1: in plan (todo) but no bead

Reconciliation Options:
  [B] Update Beads to match Conductor
  [C] Update Conductor to match Beads
  [M] Run /conductor-migrate-beads
  [S] Skip
```

---

## References

- [Beads Integration](../../skills/conductor/references/beads-integration.md) - Point 12
- [Beads Facade](../../skills/conductor/references/beads-facade.md) - API contract
- [Revise Reopen](revise-reopen-beads.md) - Reopening closed beads

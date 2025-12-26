# Beads Session Workflow

**Purpose:** Manage the complete session lifecycle for Beads-integrated Conductor commands. Handles claim, work, close, and sync operations in both SA and MA modes.

---

## Overview

This workflow covers the session lifecycle after preflight completes:

1. **Claim** - Acquire task for execution
2. **Work** - Execute with optional TDD checkpoints
3. **Close** - Complete task with reason
4. **Sync** - Push state to git

---

## Prerequisites

- Preflight completed successfully (see [preflight-beads.md](preflight-beads.md))
- Session state file exists (`.conductor/session-state_<agent>.json`)
- Mode locked (SA or MA)

---

## SA Mode Flow

```
┌────────────────────────────────────────────────────────────────┐
│                      SA MODE SESSION FLOW                       │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   bd ready --json                                               │
│        │                                                        │
│        ▼                                                        │
│   Select task from ready list                                   │
│        │                                                        │
│        ▼                                                        │
│   bd update <id> --status in_progress                           │
│        │                                                        │
│        ▼                                                        │
│   Update session state (currentTask = <id>)                     │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────────────────────────────────┐                       │
│   │  Work on task                       │                       │
│   │  (with TDD checkpoints if --tdd)    │                       │
│   └─────────────────────────────────────┘                       │
│        │                                                        │
│        ▼                                                        │
│   bd close <id> --reason <reason>                               │
│        │                                                        │
│        ▼                                                        │
│   Update session state (currentTask = null)                     │
│        │                                                        │
│        ▼                                                        │
│   bd sync (at session end)                                      │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## MA Mode Flow

```
┌────────────────────────────────────────────────────────────────┐
│                      MA MODE SESSION FLOW                       │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   init(team="<team>", role="<role>")                            │
│        │                                                        │
│        ▼                                                        │
│   inbox() ──► Check for messages                                │
│        │                                                        │
│        ▼                                                        │
│   claim() ──► Atomic task claim (race-safe)                     │
│        │                                                        │
│        ▼                                                        │
│   reserve(path="<file>") ──► Lock files before edit             │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────────────────────────────────┐                       │
│   │  Work on task                       │                       │
│   │  (with TDD checkpoints if --tdd)    │                       │
│   └─────────────────────────────────────┘                       │
│        │                                                        │
│        ▼                                                        │
│   done(taskId, reason="<reason>") ──► Auto-releases             │
│        │                                                        │
│        ▼                                                        │
│   msg(content="Task done") ──► Notify team (optional)           │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Claim Task

### SA Mode Claim

```bash
# 1. Get ready tasks
READY=$(bd ready --json)
if [[ $(echo "$READY" | jq 'length') -eq 0 ]]; then
  echo "No tasks ready. Check blockers: bd blocked --json"
  exit 0
fi

# 2. Select task (first ready or user-specified)
TASK_ID="${1:-$(echo "$READY" | jq -r '.[0].id')}"

# 3. Claim by updating status
bd update "$TASK_ID" --status in_progress
if [[ $? -ne 0 ]]; then
  echo "ERROR: Failed to claim task $TASK_ID"
  exit 1
fi

# 4. Update session state
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg task "$TASK_ID" --arg ts "$NOW" \
  '.currentTask = $task | .lastUpdated = $ts' \
  "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"

echo "Claimed: $TASK_ID"
```

### MA Mode Claim

```bash
# 1. Join team (if not already)
init team="$TEAM" role="$ROLE"

# 2. Check inbox for messages
MESSAGES=$(inbox)
if [[ -n "$MESSAGES" ]]; then
  echo "Messages from team:"
  echo "$MESSAGES"
fi

# 3. Atomic claim (prevents race conditions)
CLAIMED=$(claim)
if [[ -z "$CLAIMED" ]]; then
  echo "No tasks available to claim"
  exit 0
fi

TASK_ID=$(echo "$CLAIMED" | jq -r '.id')
echo "Claimed: $TASK_ID"
```

### Parallel Task Claiming

For executing multiple independent tasks:

```bash
# SA Mode: claim multiple tasks at once
bd update task-1 task-2 task-3 --status in_progress

# Update session state with array
jq --argjson tasks '["task-1", "task-2", "task-3"]' \
  '.currentTasks = $tasks' "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"
```

---

## Phase 2: Work with TDD Checkpoints (Opt-in)

TDD checkpoints are enabled via `--tdd` flag on `/conductor-implement`.

### Checkpoint Triggers

| Phase | Trigger | Notes Update |
|-------|---------|--------------|
| RED | Test file created or test fails | `IN_PROGRESS: RED phase - writing failing test` |
| GREEN | Test passes | `IN_PROGRESS: GREEN phase - making test pass` |
| REFACTOR | Code committed after green | `IN_PROGRESS: REFACTOR phase - cleaning up code` |

### Phase Update Flow

```bash
update_tdd_phase() {
  local TASK_ID="$1"
  local PHASE="$2"  # RED, GREEN, or REFACTOR
  
  # 1. Update session state
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  jq --arg phase "$PHASE" --arg ts "$NOW" \
    '.tddPhase = $phase | .lastUpdated = $ts' \
    "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
  mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"
  
  # 2. Update bead notes
  case "$PHASE" in
    RED)
      NOTES="IN_PROGRESS: RED phase - writing failing test"
      ;;
    GREEN)
      NOTES="IN_PROGRESS: GREEN phase - making test pass"
      ;;
    REFACTOR)
      NOTES="IN_PROGRESS: REFACTOR phase - cleaning up code"
      ;;
  esac
  
  bd update "$TASK_ID" --notes "$NOTES"
}
```

### Skip Logic

Skip TDD checkpoints when:
- `--tdd` flag not provided
- No test files detected in workspace
- Task is documentation-only

```bash
should_skip_tdd() {
  # Check for --tdd flag
  if [[ "$TDD_FLAG" != "true" ]]; then
    return 0  # Skip
  fi
  
  # Check for test files
  if ! find . -name "*_test.*" -o -name "*.test.*" -o -name "test_*" | grep -q .; then
    echo "No test files detected. Skipping TDD checkpoints."
    return 0  # Skip
  fi
  
  return 1  # Don't skip
}
```

---

## Phase 3: Close Task

### Close Reasons

| Reason | When to Use | Notes Format |
|--------|-------------|--------------|
| `completed` | Task finished successfully | `COMPLETED: <what was done>` |
| `skipped` | Task not needed (requirements changed) | `SKIPPED: <why not needed>` |
| `blocked` | Cannot proceed, external dependency | `BLOCKED: <what's blocking>` |

### SA Mode Close

```bash
close_task() {
  local TASK_ID="$1"
  local REASON="$2"
  local NOTES="$3"
  
  # 1. Update notes if provided
  if [[ -n "$NOTES" ]]; then
    bd update "$TASK_ID" --notes "$NOTES"
  fi
  
  # 2. Close with reason
  bd close "$TASK_ID" --reason "$REASON"
  CLOSE_EXIT=$?
  
  # 3. Handle failure with retry
  if [[ $CLOSE_EXIT -ne 0 ]]; then
    retry_close "$TASK_ID" "$REASON"
    return $?
  fi
  
  # 4. Update session state
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  jq --arg ts "$NOW" \
    '.currentTask = null | .tddPhase = null | .lastUpdated = $ts' \
    "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
  mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"
  
  echo "Closed: $TASK_ID ($REASON)"
}
```

### MA Mode Close

```bash
# done() auto-releases file reservations
done taskId="$TASK_ID" reason="$REASON"

# Optional: notify team
msg content="Completed $TASK_ID: $SUMMARY"
```

### Notes Format by Reason

**completed:**
```
COMPLETED: <specific deliverables>
KEY DECISION: <important choices made>
NEXT: <follow-up if any>
```

**skipped:**
```
SKIPPED: <why task was skipped>
REASON: <requirements change / not needed / superseded by X>
```

**blocked:**
```
BLOCKED: <what's preventing progress>
WAITING ON: <external dependency / decision / resource>
TRIED: <what was attempted>
```

---

## Phase 4: Sync to Git

### Sync Protocol

```bash
sync_to_git() {
  local MAX_RETRIES=3
  local RETRY_COUNT=0
  
  while [[ $RETRY_COUNT -lt $MAX_RETRIES ]]; do
    bd sync
    if [[ $? -eq 0 ]]; then
      echo "Sync: Success"
      return 0
    fi
    
    RETRY_COUNT=$((RETRY_COUNT + 1))
    echo "Sync: Attempt $RETRY_COUNT failed, retrying..."
    
    # Backoff: 1s, 2s
    sleep $RETRY_COUNT
  done
  
  # All retries failed - persist for later
  persist_unsynced
  return 1
}

persist_unsynced() {
  mkdir -p .conductor
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Get current bead IDs that may be unsynced
  bd list --json | jq -r '.[].id' > ".conductor/unsynced_beads.txt"
  
  cat > ".conductor/unsynced.json" << EOF
{
  "timestamp": "$NOW",
  "agentId": "$AGENT_ID",
  "reason": "sync_failed_after_retries"
}
EOF
  
  echo "WARN: Sync failed after $MAX_RETRIES attempts"
  echo "      Unsynced state persisted to .conductor/unsynced.json"
  echo "      Manual sync required: bd sync"
}
```

---

## Retry Logic

### Retry Schedule

```
Attempt 1: Immediate
Attempt 2: Wait 1 second
Attempt 3: Wait 2 seconds
```

### Operation-Specific Retry Behavior

| Operation | Retries | On Final Failure |
|-----------|---------|------------------|
| `bd ready` | 1x | Return empty, warn |
| `bd update` | 3x | Persist to pending_updates.jsonl |
| `bd close` | 3x | Persist to pending_closes.jsonl |
| `bd create` | 3x | HALT (cannot continue) |
| `bd sync` | 3x | Persist unsynced state |

### Pending Operations Persistence

```bash
persist_pending_update() {
  local TASK_ID="$1"
  shift
  local ARGS="$@"
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  IDEM_KEY="${AGENT_ID}_$(date +%s)_update_${TASK_ID}"
  
  mkdir -p .conductor
  
  # Append to pending operations file
  cat >> ".conductor/pending_updates.jsonl" << EOF
{"op": "update", "id": "$TASK_ID", "idempotencyKey": "$IDEM_KEY", "args": [$ARGS], "ts": "$NOW", "retries": 3}
EOF
  
  echo "WARN: Update persisted for later retry: $IDEM_KEY"
}

persist_pending_close() {
  local TASK_ID="$1"
  local REASON="$2"
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  IDEM_KEY="${AGENT_ID}_$(date +%s)_close_${TASK_ID}"
  
  mkdir -p .conductor
  
  cat >> ".conductor/pending_closes.jsonl" << EOF
{"op": "close", "id": "$TASK_ID", "idempotencyKey": "$IDEM_KEY", "args": ["--reason", "$REASON"], "ts": "$NOW", "retries": 3}
EOF
  
  echo "WARN: Close persisted for later retry: $IDEM_KEY"
}
```

### Idempotency

Before replaying pending operations, check if already applied:

```bash
is_already_applied() {
  local TASK_ID="$1"
  local EXPECTED_STATUS="$2"
  
  CURRENT_STATUS=$(bd show "$TASK_ID" --json 2>/dev/null | jq -r '.[0].status // empty')
  
  if [[ "$CURRENT_STATUS" == "$EXPECTED_STATUS" ]]; then
    return 0  # Already applied
  fi
  
  return 1  # Not yet applied
}
```

---

## Subagent Rules

When Conductor dispatches subagents via Task tool:

### Allowed (Read-Only)

```bash
# Subagents CAN use these commands:
bd show <id> --json      # View task details
bd ready --json          # See ready tasks
bd list --json           # List all tasks
bd blocked --json        # Check blockers
```

### Blocked (Write Operations)

```bash
# Subagents CANNOT use these commands:
bd update    # ❌ Return to main agent
bd close     # ❌ Return to main agent
bd create    # ❌ Return to main agent
bd sync      # ❌ Return to main agent
```

### Subagent Return Format

Subagents return structured results for main agent to process:

```json
{
  "status": "success",
  "summary": "Implemented authentication flow",
  "beadUpdates": [
    {
      "id": "bd-42",
      "action": "close",
      "reason": "completed",
      "notes": "COMPLETED: JWT auth with RS256 signing"
    }
  ],
  "filesModified": [
    "src/auth/jwt.ts",
    "src/auth/middleware.ts"
  ]
}
```

### Main Agent Processing

```bash
process_subagent_result() {
  local RESULT="$1"
  
  # Parse bead updates
  echo "$RESULT" | jq -c '.beadUpdates[]' | while read -r update; do
    ID=$(echo "$update" | jq -r '.id')
    ACTION=$(echo "$update" | jq -r '.action')
    REASON=$(echo "$update" | jq -r '.reason // "completed"')
    NOTES=$(echo "$update" | jq -r '.notes // empty')
    
    case "$ACTION" in
      close)
        if [[ -n "$NOTES" ]]; then
          bd update "$ID" --notes "$NOTES"
        fi
        bd close "$ID" --reason "$REASON"
        ;;
      update)
        bd update "$ID" --notes "$NOTES"
        ;;
    esac
  done
}
```

---

## Task Tool Injection

When dispatching subagents, inject this coordination block:

```markdown
## Beads Rules (Subagent)

You are a subagent. Beads access is READ-ONLY.

### Allowed Commands
- `bd show <id> --json` - View task details
- `bd ready --json` - See ready tasks
- `bd list --json` - List tasks

### Blocked Commands
- `bd update` - Return request to main agent
- `bd close` - Return request to main agent
- `bd create` - Return request to main agent

### Return Format
When done, return structured result:
```json
{
  "status": "success|failed",
  "summary": "Brief description of work done",
  "beadUpdates": [
    {"id": "...", "action": "close|update", "reason": "...", "notes": "..."}
  ]
}
```
```

---

## File Reservations (MA Mode)

### Reserve Before Edit

```bash
# Before editing a file
reserve path="src/auth.ts" ttl=10  # 10 minute TTL

# Edit file...

# After done (or auto-release via done())
release path="src/auth.ts"
```

### Conflict Resolution

```bash
handle_reservation_conflict() {
  local FILE="$1"
  
  # 1. Check who has the lock
  STATUS=$(status)
  HOLDER=$(echo "$STATUS" | jq -r ".reservations[\"$FILE\"].agent // empty")
  
  if [[ -z "$HOLDER" ]]; then
    echo "File not locked, trying again..."
    reserve path="$FILE"
    return $?
  fi
  
  # 2. Request access
  echo "File locked by $HOLDER. Requesting access..."
  msg to="$HOLDER" content="Need access to $FILE"
  
  # 3. Wait for response or pick different task
  echo "Options:"
  echo "  [W]ait - Check inbox for response"
  echo "  [P]ick different task"
}
```

### Auto-Release

`done()` automatically releases all reservations held by the agent:

```bash
# This releases all your file reservations:
done taskId="bd-42" reason="completed"
```

---

## Session State Updates

### State File Location

`.conductor/session-state_<agent-id>.json`

### Schema

```json
{
  "agentId": "T-abc123",
  "mode": "SA",
  "modeLockedAt": "2025-12-25T10:00:00Z",
  "trackId": "feature_20251225",
  "currentTask": "bd-42",
  "currentTasks": null,
  "tddPhase": "GREEN",
  "lastUpdated": "2025-12-25T12:00:00Z"
}
```

### Update Points

| Event | Fields Updated |
|-------|----------------|
| Claim task | `currentTask`, `lastUpdated` |
| TDD phase change | `tddPhase`, `lastUpdated` |
| Close task | `currentTask=null`, `tddPhase=null`, `lastUpdated` |
| Heartbeat | `lastUpdated` |

---

## Error Handling

| Error | Action |
|-------|--------|
| No tasks ready | Check `bd blocked --json`, report blockers |
| Claim failed | Retry 3x, then HALT with error |
| Close failed | Retry 3x, persist to pending_closes.jsonl |
| Sync failed | Retry 3x, persist unsynced state |
| Village unavailable (MA) | Degrade to SA with warning |
| Reservation conflict (MA) | Request access or pick different task |

---

## Complete Session Example

### SA Mode Session

```bash
# 1. Preflight (from preflight-beads.md)
source preflight-beads.sh

# 2. Claim
TASK_ID=$(bd ready --json | jq -r '.[0].id')
bd update "$TASK_ID" --status in_progress

# 3. Work (with TDD if enabled)
if [[ "$TDD_FLAG" == "true" ]]; then
  update_tdd_phase "$TASK_ID" "RED"
  # Write failing test...
  
  update_tdd_phase "$TASK_ID" "GREEN"
  # Make test pass...
  
  update_tdd_phase "$TASK_ID" "REFACTOR"
  # Clean up...
fi

# 4. Close
close_task "$TASK_ID" "completed" "COMPLETED: Feature implemented with tests"

# 5. Sync
sync_to_git

# 6. Cleanup
cleanup_session
```

### MA Mode Session

```bash
# 1. Preflight (from preflight-beads.md)
source preflight-beads.sh

# 2. Join team
init team="platform" role="be"

# 3. Check messages
inbox

# 4. Claim (atomic)
TASK=$(claim)
TASK_ID=$(echo "$TASK" | jq -r '.id')

# 5. Reserve files
reserve path="src/feature.ts"

# 6. Work
# ... implementation ...

# 7. Done (auto-releases reservations)
done taskId="$TASK_ID" reason="completed"

# 8. Notify team
msg content="Completed $TASK_ID"
```

---

## Handoff Protocol (MA Mode)

When one agent needs to pass context to another, use file-based handoffs.

### Handoff File Format

**Location:** `.conductor/handoff_<from>_to_<to>.json`

```json
{
  "from": "T-abc123",
  "to": "T-def456",
  "createdAt": "2025-12-25T10:00:00Z",
  "expiresAt": "2025-12-26T10:00:00Z",
  "content": "Review auth changes before merge. Key files: src/auth/*.ts",
  "context": {
    "taskId": "my-workflow:3-xyz",
    "trackId": "auth_20251225",
    "files": ["src/auth/jwt.ts", "src/auth/middleware.ts"]
  },
  "priority": "normal"
}
```

### Creating a Handoff

```bash
create_handoff() {
  local TO_AGENT="$1"
  local CONTENT="$2"
  local CONTEXT="$3"  # JSON object
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  EXPIRES=$(date -u -v+24H +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || \
            date -u -d '+24 hours' +"%Y-%m-%dT%H:%M:%SZ")
  
  HANDOFF_FILE=".conductor/handoff_${AGENT_ID}_to_${TO_AGENT}.json"
  
  cat > "$HANDOFF_FILE" << EOF
{
  "from": "$AGENT_ID",
  "to": "$TO_AGENT",
  "createdAt": "$NOW",
  "expiresAt": "$EXPIRES",
  "content": "$CONTENT",
  "context": $CONTEXT,
  "priority": "normal"
}
EOF
  
  echo "Handoff created: $HANDOFF_FILE"
}
```

### Processing Handoffs

On session start, check for incoming handoffs:

```bash
process_incoming_handoffs() {
  for handoff in .conductor/handoff_*_to_${AGENT_ID}.json; do
    [[ -f "$handoff" ]] || continue
    
    FROM=$(jq -r '.from' "$handoff")
    CONTENT=$(jq -r '.content' "$handoff")
    CREATED=$(jq -r '.createdAt' "$handoff")
    
    echo ""
    echo "📬 Handoff from $FROM (created: $CREATED):"
    echo "   $CONTENT"
    echo ""
    
    # Mark as read by moving to processed
    mkdir -p .conductor/processed_handoffs
    mv "$handoff" ".conductor/processed_handoffs/"
  done
}
```

### Handoff TTL & Cleanup

- **TTL:** 24 hours
- **Orphan Warning:** At 12 hours, preflight warns about unprocessed handoffs
- **Cleanup:** At 24 hours, expired handoffs are logged and deleted

```bash
cleanup_expired_handoffs() {
  NOW_EPOCH=$(date +%s)
  
  for handoff in .conductor/handoff_*.json; do
    [[ -f "$handoff" ]] || continue
    
    EXPIRES=$(jq -r '.expiresAt' "$handoff")
    EXPIRES_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$EXPIRES" +%s 2>/dev/null || \
                    date -d "$EXPIRES" +%s 2>/dev/null || echo 0)
    
    if [[ $NOW_EPOCH -gt $EXPIRES_EPOCH && $EXPIRES_EPOCH -gt 0 ]]; then
      echo "WARN: Expired handoff deleted:"
      cat "$handoff"
      
      # Log to metrics before deletion
      echo "{\"event\": \"handoff_expired\", \"file\": \"$handoff\", \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" >> .conductor/metrics.jsonl
      
      rm "$handoff"
    fi
  done
}
```

### Handoff Patterns

| Pattern | Use Case | Example |
|---------|----------|---------|
| **Review Request** | Need another agent to review | "Review auth changes before merge" |
| **Blocked Handoff** | Can't proceed, handing to specialist | "Blocked on DB schema, needs DBA review" |
| **Context Transfer** | Session ending, work continues | "Continuing from task X, current state: Y" |
| **Escalation** | Issue needs higher priority | "Critical bug found, needs immediate attention" |

---

## Anchored Format (SA Mode)

For single-agent sessions, use anchored format to save session context for cross-session continuity.

### Save Location

`.conductor/session-context.md`

### When to Save

- **Session end** - Before closing session
- **Token budget critical** - When >85% token usage
- **Major milestone** - After significant progress
- **Before handoff** - When switching tracks or agents

### Format Reference

→ [Anchored State Format](../../../design/references/anchored-state-format.md)

### Required Sections

| Section | [PRESERVE] | Purpose |
|---------|------------|---------|
| Intent | ✓ | What we're building and why |
| Constraints & Ruled-Out | ✓ | What we've explicitly decided NOT to do |
| Decisions Made | | Key architectural/design decisions (with Why) |
| Files Modified | | List of files touched this session |
| Open Questions / TODOs | | Things to address |
| Current State | | Where we are now |
| Next Steps | | What to do next |

### PRESERVE Validation

Before saving, validate PRESERVE sections are not empty:

```bash
validate_preserve_sections() {
  local CONTEXT_FILE="$1"
  
  # Check Intent section (using sed to extract full section)
  local intent_content
  intent_content=$(sed -n '/^## Intent/,/^## /p' "$CONTEXT_FILE" | grep -v "^##" | tr -d '[:space:]')
  if [[ -z "$intent_content" ]]; then
    echo "ERROR: Intent [PRESERVE] section is empty"
    return 1
  fi
  
  # Check Constraints section
  local constraints_content
  constraints_content=$(sed -n '/^## Constraints/,/^## /p' "$CONTEXT_FILE" | grep -v "^##" | tr -d '[:space:]')
  if [[ -z "$constraints_content" ]]; then
    echo "ERROR: Constraints [PRESERVE] section is empty"
    return 1
  fi
  
  return 0
}
```

### SA vs MA Handoff Comparison

| Aspect | SA Mode | MA Mode |
|--------|---------|---------|
| Storage | `.conductor/session-context.md` | `.conductor/handoff_*.json` |
| Format | Anchored markdown with [PRESERVE] | Structured JSON |
| Audience | Same agent, future session | Different agent, same session |
| TTL | Persistent until overwritten | 24 hours |
| Recovery | RECALL at session start | `inbox()` check |

### Integration

This extends the Handoff Protocol for SA mode. See:
- [Session Lifecycle](../../../design/references/session-lifecycle.md) - RECALL phase loads this file
- [Checkpoint Facade](checkpoint.md) - For progress checkpointing triggers

---

## References

- [Preflight Workflow](preflight-beads.md) - Session initialization
- [Beads Facade](../beads-facade.md) - API contract
- [Beads Integration](../beads-integration.md) - All 13 points
- [TDD Checkpoints](tdd-checkpoints-beads.md) - Detailed TDD workflow

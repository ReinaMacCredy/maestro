# Preflight Beads Workflow

**Purpose:** Initialize Beads integration at session/command start. Detects mode (SA/MA), validates availability, and manages session state.

---

## Overview

Preflight runs at the start of any Conductor command that interacts with Beads. It:

1. Checks `bd` CLI availability (HALT if unavailable)
2. Checks Village MCP availability (for MA mode)
3. Locks mode (SA or MA) for the session
4. Creates/recovers session state file
5. Handles concurrent session detection
6. Recovers from stale/crashed sessions

---

## Prerequisites

- `bd` CLI installed and in PATH
- Project has `.beads/` directory (or uses `~/.beads/`)
- For MA mode: Village MCP server available

---

## Mode Detection Algorithm

```
┌──────────────────────────────────────────────────────────────────┐
│                     MODE DETECTION FLOW                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│   1. Check for existing session-state file                       │
│      ├── Found + mode locked → Use existing mode                 │
│      └── Not found → Continue to step 2                          │
│                                                                   │
│   2. Check user preferences                                       │
│      ├── preferences.json has mode → Use preferred mode          │
│      └── No preference → Continue to step 3                      │
│                                                                   │
│   3. Check Village MCP availability                               │
│      ├── Available → MA mode (multi-agent)                       │
│      └── Unavailable → SA mode (single-agent)                    │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Mode Selection Precedence

1. **Existing session-state file** → use locked mode
2. **User preference** (`preferences.json`) → use preferred mode
3. **Village available + no preference** → default to MA
4. **Fallback** → SA mode

---

## Step 0: Check bd Availability

**CRITICAL:** This step HALTS if bd is unavailable. No silent skip.

```bash
# Check bd CLI exists and responds
BD_VERSION=$(bd version 2>&1)
BD_EXIT=$?

if [[ $BD_EXIT -ne 0 ]]; then
  echo "HALT: bd CLI unavailable"
  echo "Error: $BD_VERSION"
  echo ""
  echo "Install bd: https://github.com/Dicklesworthstone/beads_viewer"
  exit 1
fi

echo "Preflight: bd $BD_VERSION ✓"
```

**Timeout:** 2 seconds max for `bd version` command.

---

## Step 1: Check Existing Session State

```bash
AGENT_ID="${AGENT_ID:-$THREAD_ID}"
SESSION_FILE=".conductor/session-state_${AGENT_ID}.json"

if [[ -f "$SESSION_FILE" ]]; then
  LOCKED_MODE=$(jq -r '.mode // empty' "$SESSION_FILE")
  LOCKED_AT=$(jq -r '.modeLockedAt // empty' "$SESSION_FILE")
  LAST_UPDATED=$(jq -r '.lastUpdated // empty' "$SESSION_FILE")
  
  if [[ -n "$LOCKED_MODE" ]]; then
    echo "Session: Resuming $LOCKED_MODE mode (locked at $LOCKED_AT)"
    MODE="$LOCKED_MODE"
    # Skip to Step 4
  fi
fi
```

---

## Step 2: Check User Preferences

```bash
PREF_FILE=".conductor/preferences.json"

if [[ -f "$PREF_FILE" ]]; then
  PREFERRED_MODE=$(jq -r '.preferredMode // empty' "$PREF_FILE")
  if [[ "$PREFERRED_MODE" == "MA" || "$PREFERRED_MODE" == "SA" ]]; then
    echo "Preference: Using $PREFERRED_MODE mode"
    MODE="$PREFERRED_MODE"
    # Continue to Step 4
  fi
fi
```

---

## Step 3: Check Village MCP Availability

```bash
# Check if Village MCP server is available
VILLAGE_STATUS=$(bv --robot-status 2>&1)
VILLAGE_EXIT=$?

if [[ $VILLAGE_EXIT -eq 0 ]]; then
  echo "Village MCP: Available ✓"
  VILLAGE_AVAILABLE=1
  MODE="${MODE:-MA}"
else
  echo "Village MCP: Unavailable (SA mode)"
  VILLAGE_AVAILABLE=0
  MODE="${MODE:-SA}"
fi
```

**Note:** Use `--robot-*` flags with `bv` to avoid TUI mode.

---

## Step 4: Session Lock Check

Detect concurrent sessions on the same track.

```bash
TRACK_ID="${TRACK_ID:-}"
if [[ -n "$TRACK_ID" ]]; then
  SESSION_LOCK=".conductor/session-lock_${TRACK_ID}.json"
  
  if [[ -f "$SESSION_LOCK" ]]; then
    LOCK_AGENT=$(jq -r '.agentId // "unknown"' "$SESSION_LOCK")
    LAST_HEARTBEAT=$(jq -r '.lastHeartbeat // empty' "$SESSION_LOCK")
    
    # Check if heartbeat is stale (>10 min)
    if [[ -n "$LAST_HEARTBEAT" ]]; then
      HEARTBEAT_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_HEARTBEAT" +%s 2>/dev/null || \
                        date -d "$LAST_HEARTBEAT" +%s 2>/dev/null || echo 0)
      NOW_EPOCH=$(date +%s)
      STALE_THRESHOLD=$((NOW_EPOCH - 600))  # 10 minutes
      
      if [[ $HEARTBEAT_EPOCH -lt $STALE_THRESHOLD && $HEARTBEAT_EPOCH -gt 0 ]]; then
        # Stale lock - auto-unlock
        echo "WARN: Stale session lock detected (no heartbeat for >10 min)"
        echo "      Previous agent: $LOCK_AGENT"
        rm "$SESSION_LOCK"
        echo "      Auto-removed stale lock"
      else
        # Active lock - prompt user
        echo ""
        echo "⚠️  ACTIVE SESSION DETECTED"
        echo "   Track: $TRACK_ID"
        echo "   Agent: $LOCK_AGENT"
        echo "   Last heartbeat: $LAST_HEARTBEAT"
        echo ""
        echo "Options:"
        echo "  [C]ontinue - Proceed anyway (risk conflicts)"
        echo "  [W]ait - Wait for other session to finish"
        echo "  [F]orce - Force unlock (other session will error)"
        echo ""
        # Require explicit user action
      fi
    fi
  fi
fi
```

---

## Step 5: Create/Update Session State

```bash
mkdir -p .conductor

# Create session state file
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
cat > "$SESSION_FILE" << EOF
{
  "agentId": "$AGENT_ID",
  "mode": "$MODE",
  "modeLockedAt": "$NOW",
  "trackId": ${TRACK_ID:+\"$TRACK_ID\"}${TRACK_ID:-null},
  "currentTask": null,
  "tddPhase": null,
  "lastUpdated": "$NOW"
}
EOF

echo "Session: Created state file for $AGENT_ID ($MODE mode)"

# Create session lock if working on a track (atomic using mkdir)
if [[ -n "$TRACK_ID" ]]; then
  LOCK_DIR=".conductor/session-lock_${TRACK_ID}.lock"
  
  # mkdir is atomic - only one agent can create the directory
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    # We won the race - write our lock file
    cat > "$SESSION_LOCK" << EOF
{
  "agentId": "$AGENT_ID",
  "lockedAt": "$NOW",
  "lastHeartbeat": "$NOW",
  "trackId": "$TRACK_ID",
  "pid": $$
}
EOF
    # Remove the lock directory (lock file is the real lock now)
    rmdir "$LOCK_DIR"
  else
    # Another agent is creating the lock - wait briefly and re-check
    sleep 1
    if [[ -f "$SESSION_LOCK" ]]; then
      LOCK_AGENT=$(jq -r '.agentId // "unknown"' "$SESSION_LOCK")
      echo "WARN: Race condition detected - another agent ($LOCK_AGENT) acquired lock"
      echo "      Re-run preflight or use --force to override"
      exit 1
    fi
  fi
fi
```

---

## Step 6: Recover Pending Operations

Check for and replay any pending operations from crashed sessions.

```bash
# Check for pending updates
if [[ -f ".conductor/pending_updates.jsonl" ]]; then
  PENDING_COUNT=$(wc -l < ".conductor/pending_updates.jsonl")
  echo "WARN: Found $PENDING_COUNT pending update operations"
  echo "      Replaying..."
  
  while IFS= read -r line; do
    OP=$(echo "$line" | jq -r '.op')
    ID=$(echo "$line" | jq -r '.id')
    ARGS=$(echo "$line" | jq -r '.args | join(" ")')
    IDEM_KEY=$(echo "$line" | jq -r '.idempotencyKey')
    
    # Check idempotency - skip if already applied
    CURRENT_STATUS=$(bd show "$ID" --json 2>/dev/null | jq -r '.[0].status // empty')
    TARGET_STATUS=$(echo "$ARGS" | grep -o 'in_progress\|closed\|open' || echo "")
    
    if [[ "$CURRENT_STATUS" == "$TARGET_STATUS" ]]; then
      echo "      Skip (already applied): $IDEM_KEY"
    else
      echo "      Replaying: $IDEM_KEY"
      bd update "$ID" $ARGS
    fi
  done < ".conductor/pending_updates.jsonl"
  
  rm ".conductor/pending_updates.jsonl"
  echo "      Pending updates replayed"
fi

# Check for pending closes
if [[ -f ".conductor/pending_closes.jsonl" ]]; then
  PENDING_COUNT=$(wc -l < ".conductor/pending_closes.jsonl")
  echo "WARN: Found $PENDING_COUNT pending close operations"
  echo "      Replaying..."
  
  while IFS= read -r line; do
    ID=$(echo "$line" | jq -r '.id')
    REASON=$(echo "$line" | jq -r '.args[1] // "completed"')
    IDEM_KEY=$(echo "$line" | jq -r '.idempotencyKey')
    
    CURRENT_STATUS=$(bd show "$ID" --json 2>/dev/null | jq -r '.[0].status // empty')
    if [[ "$CURRENT_STATUS" == "closed" ]]; then
      echo "      Skip (already closed): $IDEM_KEY"
    else
      echo "      Replaying: $IDEM_KEY"
      bd close "$ID" --reason "$REASON"
    fi
  done < ".conductor/pending_closes.jsonl"
  
  rm ".conductor/pending_closes.jsonl"
  echo "      Pending closes replayed"
fi
```

---

## Step 7: Check Orphan Handoffs (MA Mode)

```bash
if [[ "$MODE" == "MA" ]]; then
  # Check for orphan handoff files (>12 hours old)
  TWELVE_HOURS_AGO=$(date -v-12H +%s 2>/dev/null || date -d '12 hours ago' +%s)
  
  for handoff in .conductor/handoff_*.json; do
    [[ -f "$handoff" ]] || continue
    
    FILE_MTIME=$(stat -f %m "$handoff" 2>/dev/null || stat -c %Y "$handoff")
    if [[ $FILE_MTIME -lt $TWELVE_HOURS_AGO ]]; then
      FROM=$(basename "$handoff" | sed 's/handoff_\(.*\)_to_.*/\1/')
      TO=$(basename "$handoff" | sed 's/.*_to_\(.*\)\.json/\1/')
      CONTENT=$(jq -r '.content // "No content"' "$handoff")
      
      echo ""
      echo "⚠️ Orphan handoff detected:"
      echo "   From: $FROM → To: $TO"
      echo "   Age: >12 hours"
      echo "   Content: $CONTENT"
      echo "   Action: Target agent has not been online. Consider reassigning."
      echo ""
    fi
  done
fi
```

---

## Step 8: Stale Agent Detection (MA Mode)

```bash
if [[ "$MODE" == "MA" ]]; then
  TEN_MINUTES_AGO=$(date -v-10M +%s 2>/dev/null || date -d '10 minutes ago' +%s)
  
  for state_file in .conductor/session-state_*.json; do
    [[ -f "$state_file" ]] || continue
    [[ "$state_file" == "$SESSION_FILE" ]] && continue  # Skip self
    
    OTHER_AGENT=$(jq -r '.agentId // "unknown"' "$state_file")
    LAST_UPDATED=$(jq -r '.lastUpdated // empty' "$state_file")
    
    if [[ -n "$LAST_UPDATED" ]]; then
      UPDATED_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_UPDATED" +%s 2>/dev/null || \
                      date -d "$LAST_UPDATED" +%s 2>/dev/null || echo 0)
      
      if [[ $UPDATED_EPOCH -lt $TEN_MINUTES_AGO && $UPDATED_EPOCH -gt 0 ]]; then
        CURRENT_TASK=$(jq -r '.currentTask // "none"' "$state_file")
        echo "INFO: Stale agent detected: $OTHER_AGENT (last seen: $LAST_UPDATED)"
        echo "      Current task: $CURRENT_TASK"
      fi
    fi
  done
fi
```

---

## Output Format

### Success Output

```
Preflight: bd v0.5.2 ✓, Village ✗ → SA mode
Session: Created state file for T-abc123 (SA mode)
```

### With Recovery

```
Preflight: bd v0.5.2 ✓, Village ✓ → MA mode
WARN: Found 2 pending update operations
      Replaying...
      Replaying: T-abc_1703509200_update_bd-42
      Skip (already applied): T-abc_1703509260_update_bd-43
      Pending updates replayed
Session: Resuming MA mode (locked at 2025-12-25T10:00:00Z)
```

### With Active Session Warning

```
Preflight: bd v0.5.2 ✓

⚠️  ACTIVE SESSION DETECTED
   Track: beads-integration_20251225
   Agent: T-def456
   Last heartbeat: 2025-12-25T10:25:00Z

Options:
  [C]ontinue - Proceed anyway (risk conflicts)
  [W]ait - Wait for other session to finish
  [F]orce - Force unlock (other session will error)
```

---

## Heartbeat Protocol

Active sessions update heartbeat every 5 minutes:

```bash
update_heartbeat() {
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Update session state
  if [[ -f "$SESSION_FILE" ]]; then
    jq --arg ts "$NOW" '.lastUpdated = $ts' "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
    mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"
  fi
  
  # Update session lock
  if [[ -f "$SESSION_LOCK" ]]; then
    jq --arg ts "$NOW" '.lastHeartbeat = $ts' "$SESSION_LOCK" > "$SESSION_LOCK.tmp.$$"
    mv "$SESSION_LOCK.tmp.$$" "$SESSION_LOCK"
  fi
}
```

### Background Heartbeat Loop

Run this in the background to keep the session alive:

```bash
start_heartbeat_loop() {
  (
    while [[ -f "$SESSION_LOCK" ]]; do
      sleep 300  # 5 minutes
      update_heartbeat
    done
  ) &
  HEARTBEAT_PID=$!
}

stop_heartbeat_loop() {
  if [[ -n "${HEARTBEAT_PID:-}" ]]; then
    kill "$HEARTBEAT_PID" 2>/dev/null || true
    unset HEARTBEAT_PID
  fi
}

# Usage:
# start_heartbeat_loop  # Call at session start
# stop_heartbeat_loop   # Call at session end (cleanup)
```

**Schedule:** Called at 5-minute intervals during active session.

---

## Metrics Logging

Log usage events to `.conductor/metrics.jsonl` for analysis:

```bash
log_metric() {
  local EVENT="$1"
  local EXTRA_JSON="${2:-}"
  
  METRICS_FILE=".conductor/metrics.jsonl"
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  mkdir -p .conductor
  
  # Build JSON with optional extra fields
  if [[ -n "$EXTRA_JSON" ]]; then
    echo "{\"event\": \"$EVENT\", $EXTRA_JSON, \"timestamp\": \"$NOW\"}" >> "$METRICS_FILE"
  else
    echo "{\"event\": \"$EVENT\", \"timestamp\": \"$NOW\"}" >> "$METRICS_FILE"
  fi
}

# Log during preflight
log_preflight_metrics() {
  # Log mode detection
  log_metric "ma_attempt" "\"mode\": \"$MODE\", \"village_available\": $VILLAGE_AVAILABLE"
  
  # Log if recovering from crashed session
  if [[ -f ".conductor/pending_updates.jsonl" || -f ".conductor/pending_closes.jsonl" ]]; then
    log_metric "pending_recovery" "\"updates\": $(wc -l < .conductor/pending_updates.jsonl 2>/dev/null || echo 0), \"closes\": $(wc -l < .conductor/pending_closes.jsonl 2>/dev/null || echo 0)"
  fi
}
```

### Event Types

| Event | When Logged | Extra Fields |
|-------|-------------|--------------|
| `ma_attempt` | Preflight mode detection | `mode`, `village_available` |
| `pending_recovery` | Replaying pending operations | `updates`, `closes` |
| `tdd_cycle` | TDD phase transition | `taskId`, `phase`, `duration` |
| `manual_bd` | Manual bd command (outside Conductor) | `command` |
| `handoff_expired` | Handoff file TTL exceeded | `file` |
| `session_conflict` | Concurrent session detected | `track_id`, `other_agent` |

### Viewing Metrics

Run the metrics summary script:

```bash
./scripts/beads-metrics-summary.sh          # Last 7 days
./scripts/beads-metrics-summary.sh --days 30  # Last 30 days
```

---

## Session End Cleanup

```bash
cleanup_session() {
  # Remove session lock
  if [[ -f "$SESSION_LOCK" ]]; then
    rm "$SESSION_LOCK"
  fi
  
  # Update session state (keep for crash recovery)
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  if [[ -f "$SESSION_FILE" ]]; then
    jq --arg ts "$NOW" '.lastUpdated = $ts | .currentTask = null' "$SESSION_FILE" > "$SESSION_FILE.tmp.$$"
    mv "$SESSION_FILE.tmp.$$" "$SESSION_FILE"
  fi
  
  # Sync beads to git
  bd sync
}
```

---

## Error Handling

| Error | Action |
|-------|--------|
| `bd` unavailable | HALT with install instructions |
| `bd version` times out | HALT with timeout message |
| Session lock active (<10 min) | Prompt C/W/F |
| Session lock stale (>10 min) | Auto-unlock + warn |
| Pending operations exist | Replay with idempotency check |
| Village unavailable (MA mode) | Degrade to SA + warn |

---

## HALT vs Degrade

| Condition | Action |
|-----------|--------|
| bd unavailable | HALT |
| Village unavailable, started as SA | Continue SA |
| Village unavailable, started as MA | Degrade to SA with warning |
| bd fails mid-session | Retry 3x → persist → warn |

**Degraded MA Mode Warning:**
```
⚠️ Village MCP unavailable. Operating in degraded mode.
- File reservations: SKIPPED (cannot enforce)
- Task claiming: Using bd update (no atomic guarantee)
- Handoffs: Written to .conductor/handoff_*.json
```

---

## State Files

| File | Location | Purpose |
|------|----------|---------|
| `session-state_<agent-id>.json` | `.conductor/` | Per-agent session tracking |
| `session-lock_<track-id>.json` | `.conductor/` | Concurrent session prevention |
| `preferences.json` | `.conductor/` | User mode preferences |
| `pending_updates.jsonl` | `.conductor/` | Failed update ops for replay |
| `pending_closes.jsonl` | `.conductor/` | Failed close ops for replay |
| `handoff_<from>_to_<to>.json` | `.conductor/` | MA handoff messages |

---

## Integration with Conductor Commands

### Called By

All Conductor commands that interact with Beads:

- `/conductor-implement`
- `/conductor-newtrack` (when filing beads)
- `/conductor-status`
- `/conductor-finish`
- `/conductor-revise`

### How to Call

```markdown
# At start of any Beads-integrated command

## Phase 0: Preflight

Run preflight-beads workflow:
1. Check bd availability
2. Detect/lock mode
3. Create session state
4. Recover pending operations
5. Check for conflicts

If preflight fails → HALT command.
```

---

## References

- [Beads Facade](../skills/conductor/references/beads-facade.md) - API contract
- [Beads Integration](../skills/conductor/references/beads-integration.md) - All 13 points
- [Track Validation](../skills/conductor/references/validation/track/checks.md) - Session lock detection

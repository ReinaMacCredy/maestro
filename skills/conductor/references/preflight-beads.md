# Preflight Beads Workflow

**Purpose:** Initialize Beads integration at session/command start. Validates availability, registers with Agent Mail, and manages session state.

---

## Overview

Preflight runs at the start of any Conductor command that interacts with Beads. It:

1. Checks `bd` CLI availability (HALT if unavailable)
2. Checks Agent Mail availability (HALT if unavailable)
3. Registers agent with Agent Mail
4. Creates/recovers session state in metadata.json
5. Handles concurrent session detection
6. Recovers from stale/crashed sessions

---

## Prerequisites

- `bd` CLI installed and in PATH
- Project has `.beads/` directory (or uses `~/.beads/`)
- Agent Mail MCP server available

---

## Session Initialization Flow

```text
┌──────────────────────────────────────────────────────────────────┐
│                   SESSION INITIALIZATION FLOW                     │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│   1. Check bd CLI availability                                    │
│      ├── Available → Continue                                     │
│      └── Unavailable → HALT                                       │
│                                                                   │
│   2. Check Agent Mail availability                                │
│      ├── Available → Continue                                     │
│      └── Unavailable → HALT                                       │
│                                                                   │
│   3. Register agent with Agent Mail                               │
│      └── ensure_project + register_agent                          │
│                                                                   │
│   4. Load/create session state in metadata.json                   │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

---

## Step 0: Check Cached Beads State

Before running any triage or discovery, check if beads have already been filed for this track.

### Triage Cache Structure

The triage cache is stored in `metadata.beads.triageCache`:

```json
{
  "beads": {
    "status": "complete",
    "triageCache": {
      "cachedAt": "2026-01-02T12:00:00Z",
      "ttlSeconds": 3600,
      "results": [
        {
          "id": "my-workflow:3-abc1",
          "status": "ready",
          "title": "Task 1",
          "dependencies": []
        }
      ],
      "counts": {
        "ready": 3,
        "in_progress": 1,
        "blocked": 0,
        "closed": 2
      }
    }
  }
}
```

### Check Cache Before Triage

```bash
TRACK_ID="${TRACK_ID:-}"
SKIP_TRIAGE=false
CACHED_RESULTS=""

if [[ -n "$TRACK_ID" ]]; then
  METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
  
  if [[ -f "$METADATA_FILE" ]]; then
    BEADS_STATUS=$(jq -r '.beads.status // empty' "$METADATA_FILE")
    
    if [[ "$BEADS_STATUS" == "complete" ]]; then
      # Check if triage cache exists and is valid
      CACHE_EXISTS=$(jq -r '.beads.triageCache.cachedAt // empty' "$METADATA_FILE")
      
      if [[ -n "$CACHE_EXISTS" ]]; then
        CACHED_AT=$(jq -r '.beads.triageCache.cachedAt' "$METADATA_FILE")
        TTL_SECONDS=$(jq -r '.beads.triageCache.ttlSeconds // 3600' "$METADATA_FILE")
        
        # Calculate cache age
        CACHED_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$CACHED_AT" +%s 2>/dev/null || \
                       date -d "$CACHED_AT" +%s 2>/dev/null || echo 0)
        NOW_EPOCH=$(date +%s)
        CACHE_AGE=$((NOW_EPOCH - CACHED_EPOCH))
        
        if [[ $CACHE_AGE -lt $TTL_SECONDS ]]; then
          echo "✓ Using cached triage state from metadata.json"
          echo "  Cache age: ${CACHE_AGE}s (TTL: ${TTL_SECONDS}s)"
          echo "  Beads already filed for track: $TRACK_ID"
          
          # Read cached results for use in subsequent steps
          CACHED_RESULTS=$(jq -c '.beads.triageCache.results' "$METADATA_FILE")
          SKIP_TRIAGE=true
        else
          echo "⚠ Triage cache expired (age: ${CACHE_AGE}s > TTL: ${TTL_SECONDS}s)"
          echo "  Will refresh triage data..."
        fi
      else
        echo "✓ Using cached bead state from metadata.json"
        echo "  Beads already filed for track: $TRACK_ID"
        # Skip bv --robot-triage - beads are already populated
        SKIP_TRIAGE=true
      fi
    fi
  fi
fi
```

### Store Triage Results in Cache

After running `bv --robot-triage`, store results in the cache:

```bash
store_triage_cache() {
  local TRACK_ID="$1"
  local TRIAGE_OUTPUT="$2"
  local TTL_SECONDS="${3:-3600}"  # Default 1 hour TTL
  
  METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
  
  if [[ ! -f "$METADATA_FILE" ]]; then
    echo "ERROR: metadata.json not found for track: $TRACK_ID"
    return 1
  fi
  
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  TEMP_FILE="$METADATA_FILE.tmp.$$"
  
  # Parse triage output and compute counts
  READY_COUNT=$(echo "$TRIAGE_OUTPUT" | jq '[.[] | select(.status == "ready")] | length')
  IN_PROGRESS_COUNT=$(echo "$TRIAGE_OUTPUT" | jq '[.[] | select(.status == "in_progress")] | length')
  BLOCKED_COUNT=$(echo "$TRIAGE_OUTPUT" | jq '[.[] | select(.status == "blocked")] | length')
  CLOSED_COUNT=$(echo "$TRIAGE_OUTPUT" | jq '[.[] | select(.status == "closed")] | length')
  
  # Update metadata.json with triage cache
  jq --arg now "$NOW" \
     --argjson ttl "$TTL_SECONDS" \
     --argjson results "$TRIAGE_OUTPUT" \
     --argjson ready "$READY_COUNT" \
     --argjson in_progress "$IN_PROGRESS_COUNT" \
     --argjson blocked "$BLOCKED_COUNT" \
     --argjson closed "$CLOSED_COUNT" \
     '.beads.triageCache = {
        cachedAt: $now,
        ttlSeconds: $ttl,
        results: $results,
        counts: {
          ready: $ready,
          in_progress: $in_progress,
          blocked: $blocked,
          closed: $closed
        }
      }' "$METADATA_FILE" > "$TEMP_FILE"
  
  mv "$TEMP_FILE" "$METADATA_FILE"
  echo "✓ Triage cache stored (TTL: ${TTL_SECONDS}s)"
}

# Usage after bv --robot-triage:
# TRIAGE_OUTPUT=$(bv --robot-triage 2>/dev/null)
# if [[ $? -eq 0 && -n "$TRIAGE_OUTPUT" ]]; then
#   store_triage_cache "$TRACK_ID" "$TRIAGE_OUTPUT"
# fi
```

### Invalidate Cache on Bead State Change

```bash
invalidate_triage_cache() {
  local TRACK_ID="$1"
  
  METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
  
  if [[ -f "$METADATA_FILE" ]]; then
    TEMP_FILE="$METADATA_FILE.tmp.$$"
    
    jq 'del(.beads.triageCache)' "$METADATA_FILE" > "$TEMP_FILE"
    mv "$TEMP_FILE" "$METADATA_FILE"
    
    echo "✓ Triage cache invalidated"
  fi
}

# Call invalidate_triage_cache when:
# - bd update changes bead status
# - bd close is called
# - New beads are filed
```

---

## Step 0b: Check bd Availability

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

Session state is stored in metadata.json at `conductor/tracks/<track_id>/metadata.json`.

```bash
AGENT_ID="${AGENT_ID:-$THREAD_ID}"
TRACK_ID="${TRACK_ID:-}"

if [[ -n "$TRACK_ID" ]]; then
  METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
  
  if [[ -f "$METADATA_FILE" ]]; then
    # Extract session fields from metadata.json
    LOCKED_MODE=$(jq -r '.session.mode // empty' "$METADATA_FILE")
    LAST_ACTIVITY=$(jq -r '.last_activity // empty' "$METADATA_FILE")
    BOUND_BEAD=$(jq -r '.session.bound_bead // empty' "$METADATA_FILE")
    
    if [[ -n "$LOCKED_MODE" && "$LOCKED_MODE" != "null" ]]; then
      echo "Session: Resuming $LOCKED_MODE mode (track: $TRACK_ID)"
      MODE="$LOCKED_MODE"
      # Skip to Step 4
    fi
  fi
fi
```

---

## Step 2: Check Agent Mail Availability

```python
try:
    ensure_project(human_key=PROJECT_PATH)
    register_agent(
        project_key=PROJECT_PATH,
        program="amp",
        model="claude-sonnet-4-20250514"
    )
    print("Agent Mail: Available ✓")
except McpUnavailable:
    print("HALT: Agent Mail unavailable")
    exit(1)
```

---

## Step 3: Session Lock Check

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

## Step 4: Update Session State

Session state is stored in metadata.json. Update it and touch activity marker:

```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

if [[ -n "$TRACK_ID" ]]; then
  METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
  
  if [[ -f "$METADATA_FILE" ]]; then
    # Update metadata.json with session fields
    TEMP_FILE="$METADATA_FILE.tmp.$$"
    
    jq --arg now "$NOW" \
       --arg agent "$AGENT_ID" \
       '. + {
         last_activity: $now,
         session: {
           agent_id: $agent,
           bound_bead: (.session.bound_bead // null),
           tdd_phase: (.session.tdd_phase // null),
           started_at: (.session.started_at // $now)
         }
       }' "$METADATA_FILE" > "$TEMP_FILE"
    
    mv "$TEMP_FILE" "$METADATA_FILE"
  fi
fi

# Touch global activity marker
mkdir -p conductor
touch conductor/.last_activity

echo "Session: metadata.json updated for $AGENT_ID"
```

---

## Step 4b: RECALL Session Context

Load prior session context to enable cross-session continuity.

### Load Context from Handoffs

```bash
HANDOFF_DIR="conductor/handoffs/${TRACK_ID}"

if [[ -n "$TRACK_ID" && -d "$HANDOFF_DIR" ]]; then
  # Find most recent handoff
  LATEST_HANDOFF=$(ls -t "$HANDOFF_DIR"/*.md 2>/dev/null | head -1)
  
  if [[ -n "$LATEST_HANDOFF" ]]; then
    echo "RECALL: Loading handoff context..."
    
    # Display key context from handoff
    INTENT=$(sed -n '/## Intent/,/^## /p' "$LATEST_HANDOFF" | head -10)
    CURRENT_STATE=$(sed -n '/## Current State/,/^## /p' "$LATEST_HANDOFF" | head -5)
    
    echo "   Intent: $(echo "$INTENT" | head -2 | tail -1)"
    echo "   State: $(echo "$CURRENT_STATE" | head -2 | tail -1)"
    echo "RECALL: Context loaded ✓"
  fi
else
  echo "RECALL: No prior handoff found"
fi
```

### Context Contract Validation

```bash
validate_context_contract() {
  local FILE="$1"
  
  # Check required PRESERVE sections
  if ! grep -q "## Intent" "$FILE"; then
    echo "ERROR: Missing Intent [PRESERVE] section"
    return 1
  fi
  
  if ! grep -q "## Constraints" "$FILE"; then
    echo "ERROR: Missing Constraints [PRESERVE] section"
    return 1
  fi
  
  # Check Intent is not empty
  INTENT_CONTENT=$(sed -n '/## Intent/,/^## /p' "$FILE" | grep -v "^## " | tr -d '[:space:]')
  if [[ -z "$INTENT_CONTENT" ]]; then
    echo "ERROR: Intent section is empty"
    return 1
  fi
  
  return 0
}
```

### Token Budget Display

```bash
display_token_budget() {
  # Note: Token budget is tracked by the agent runtime, not this script
  # This is a documentation of what the agent should display
  
  echo ""
  echo "┌─ TOKEN BUDGET ─────────────────────────┐"
  echo "│ Available:  [from runtime]             │"
  echo "│ Prompt:     [from runtime]             │"
  echo "│ Reserved:   [from runtime]             │"
  echo "│ Usable:     [calculated]               │"
  echo "├─────────────────────────────────────────┤"
  echo "│ Status:     [OK/WARN/CRITICAL]         │"
  echo "└─────────────────────────────────────────┘"
  
  # Thresholds:
  # - <20% usable → WARN (suggest checkpoint)
  # - <10% usable → CRITICAL (force compression)
}
```

### Integration

> Load the [designing skill](../../designing/SKILL.md) for Anchored State Format template and Session Lifecycle (RECALL/ROUTE flow)

---

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
  
  for handoff_dir in conductor/handoffs/*/; do
    [[ -d "$handoff_dir" ]] || continue
    
    for handoff in "$handoff_dir"*.md; do
      [[ -f "$handoff" ]] || continue
      
      FILE_MTIME=$(stat -f %m "$handoff" 2>/dev/null || stat -c %Y "$handoff")
      if [[ $FILE_MTIME -lt $TWELVE_HOURS_AGO ]]; then
        TRACK=$(basename "$(dirname "$handoff")")
        
        echo ""
        echo "⚠️ Orphan handoff detected:"
        echo "   Track: $TRACK"
        echo "   File: $(basename "$handoff")"
        echo "   Age: >12 hours"
        echo "   Action: Consider archiving or resuming."
        echo ""
      fi
    done
  done
fi
```

---

## Step 8: Stale Agent Detection (MA Mode)

In MA mode, stale agents are detected via session lock heartbeat. Detection relies on session lock files for concurrent session prevention.

```bash
if [[ "$MODE" == "MA" ]]; then
  TEN_MINUTES_AGO=$(date -v-10M +%s 2>/dev/null || date -d '10 minutes ago' +%s)
  
  # Check session lock files for stale agents
  for lock_file in .conductor/session-lock_*.json; do
    [[ -f "$lock_file" ]] || continue
    
    OTHER_AGENT=$(jq -r '.agentId // "unknown"' "$lock_file")
    [[ "$OTHER_AGENT" == "$AGENT_ID" ]] && continue  # Skip self
    
    LAST_HEARTBEAT=$(jq -r '.lastHeartbeat // empty' "$lock_file")
    
    if [[ -n "$LAST_HEARTBEAT" ]]; then
      HEARTBEAT_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_HEARTBEAT" +%s 2>/dev/null || \
                        date -d "$LAST_HEARTBEAT" +%s 2>/dev/null || echo 0)
      
      if [[ $HEARTBEAT_EPOCH -lt $TEN_MINUTES_AGO && $HEARTBEAT_EPOCH -gt 0 ]]; then
        TRACK_ID=$(jq -r '.trackId // "unknown"' "$lock_file")
        echo "INFO: Stale agent detected: $OTHER_AGENT (last heartbeat: $LAST_HEARTBEAT)"
        echo "      Track: $TRACK_ID"
      fi
    fi
  done
fi
```

---

## Output Format

### Success Output

```text
Preflight: bd v0.5.2 ✓, Agent Mail ✓
Session: metadata.json updated for T-abc123
```

### With Recovery

```text
Preflight: bd v0.5.2 ✓, Agent Mail ✓
WARN: Found 2 pending update operations
      Replaying...
      Replaying: T-abc_1703509200_update_bd-42
      Skip (already applied): T-abc_1703509260_update_bd-43
      Pending updates replayed
Session: Resuming track: auth_20251225
```

### With Active Session Warning

```text
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

Active sessions update heartbeat every 5 minutes via activity marker:

```bash
update_heartbeat() {
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  # Touch activity marker
  touch conductor/.last_activity
  
  # Update metadata.json if track is bound
  if [[ -n "$TRACK_ID" ]]; then
    METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
    
    if [[ -f "$METADATA_FILE" ]]; then
      TEMP_FILE="$METADATA_FILE.tmp.$$"
      
      jq --arg now "$NOW" '.last_activity = $now' "$METADATA_FILE" > "$TEMP_FILE"
      mv "$TEMP_FILE" "$METADATA_FILE"
    fi
  fi
  
  # Update session lock (still JSON for concurrent session detection)
  SESSION_LOCK=".conductor/session-lock_${TRACK_ID}.json"
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
    while [[ -f "conductor/.last_activity" ]]; do
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
  # Log session start
  log_metric "session_start" "\"agent_id\": \"$AGENT_ID\""
  
  # Log if recovering from crashed session
  if [[ -f ".conductor/pending_updates.jsonl" || -f ".conductor/pending_closes.jsonl" ]]; then
    log_metric "pending_recovery" "\"updates\": $(wc -l < .conductor/pending_updates.jsonl 2>/dev/null || echo 0), \"closes\": $(wc -l < .conductor/pending_closes.jsonl 2>/dev/null || echo 0)"
  fi
}
```

### Event Types

| Event | When Logged | Extra Fields |
|-------|-------------|--------------|
| `session_start` | Preflight complete | `agent_id` |
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
  SESSION_LOCK=".conductor/session-lock_${TRACK_ID}.json"
  
  # Remove session lock
  if [[ -f "$SESSION_LOCK" ]]; then
    rm "$SESSION_LOCK"
  fi
  
  # Clear session fields in metadata.json (keep for continuity, clear active state)
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  if [[ -n "$TRACK_ID" ]]; then
    METADATA_FILE="conductor/tracks/${TRACK_ID}/metadata.json"
    
    if [[ -f "$METADATA_FILE" ]]; then
      TEMP_FILE="$METADATA_FILE.tmp.$$"
      
      jq --arg now "$NOW" \
         '.last_activity = $now | .session.bound_bead = null | .session.tdd_phase = null' \
         "$METADATA_FILE" > "$TEMP_FILE"
      
      mv "$TEMP_FILE" "$METADATA_FILE"
    fi
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
| Agent Mail unavailable | HALT |

---

## HALT Conditions

| Condition | Action |
|-----------|--------|
| bd unavailable | HALT with install instructions |
| Agent Mail unavailable | HALT - required for coordination |
| bd fails mid-session | Retry 3x → persist → warn |

---

## State Files

| File | Location | Purpose |
|------|----------|---------|
| `metadata.json` | `conductor/tracks/<track>/` | Track + session state (bound_bead, tdd_phase, last_activity) |
| `.last_activity` | `conductor/` | Global activity marker (touch for heartbeat) |
| `session-lock_<track-id>.json` | `.conductor/` | Concurrent session prevention |
| `pending_updates.jsonl` | `.conductor/` | Failed update ops for replay |
| `pending_closes.jsonl` | `.conductor/` | Failed close ops for replay |
| Handoff files | `conductor/handoffs/<track>/` | Context preservation between sessions |

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
3. Update metadata.json session state
4. Recover pending operations
5. Check for conflicts

If preflight fails → HALT command.
```

---

## References

- [Beads Facade](beads-facade.md) - API contract
- [Beads Integration](beads-integration.md) - All 13 points
- [Track Validation](../validation/track-checks.md) - Session lock detection

# Multi-Session Integration Test

Manual test procedure for verifying session coordination.

## Prerequisites

- Two terminal windows
- Agent Mail MCP running
- Clean beads state: `bd list --status=open`

## Test 1: Session Detection

### Steps

**Terminal 1:**
1. Run `/conductor-implement session-brain`
2. Verify: Shows "no active sessions" or current sessions
3. Keep session running

**Terminal 2:**
1. Run `/conductor-implement session-brain`
2. Verify: Shows Terminal 1's session in "ACTIVE SESSIONS"

### Expected Result
Terminal 2 sees Terminal 1 as active session with track, beads, files.

## Test 2: Track Conflict Warning

### Steps

**Terminal 1:**
1. Run `/conductor-implement session-brain`
2. Keep running

**Terminal 2:**
1. Run `/conductor-implement session-brain` (same track)
2. Verify: Shows "TRACK CONFLICT" warning
3. Choose [P]roceed

### Expected Result
Conflict prompt appears with [P]roceed/[S]witch/[W]ait options.

## Test 3: Stale Session Takeover

### Steps

**Terminal 1:**
1. Run `/conductor-implement session-brain`
2. Wait 12 minutes (or mock last_heartbeat)
3. DO NOT interact

**Terminal 2:**
1. Run `/conductor-implement session-brain`
2. Verify: Shows "STALE SESSION DETECTED"
3. Choose [T]ake over

### Expected Result
- Stale prompt with [T]ake/[W]ait/[I]gnore
- On takeover: reservations released, beads reset

## Test 4: ds Skips Preflight

### Steps

1. Run `ds`
2. Verify: No session registration, no "ACTIVE SESSIONS" display

### Expected Result
Design session starts immediately without preflight.

## Test 5: Agent Mail Timeout

### Steps

1. Stop Agent Mail MCP
2. Run `/conductor-implement session-brain`
3. Wait 3 seconds

### Expected Result
Warning: "Agent Mail unavailable - proceeding without coordination"

## Test 6: File Reservation Conflict

### Steps

**Terminal 1:**
1. Run `/conductor-implement session-brain`
2. Reserve files: `skills/orchestrator/**`
3. Keep running

**Terminal 2:**
1. Run `/conductor-implement session-brain` with file scope `skills/orchestrator/scripts/**`
2. Verify: Shows file overlap warning

### Expected Result
File conflict detected showing overlapping glob patterns.

## Test 7: Heartbeat Updates

### Steps

**Terminal 1:**
1. Run `/conductor-implement session-brain`
2. Work for 10+ minutes
3. Verify heartbeat messages in Agent Mail

**Terminal 2:**
1. Periodically check active sessions
2. Verify last_heartbeat updates every 5 minutes

### Expected Result
Heartbeat timestamp updates regularly, preventing false stale detection.

## Verification Commands

```bash
# Check active sessions (empty inbox)
python skills/orchestrator/scripts/preflight.py detect '[]'

# Format session display
python skills/orchestrator/scripts/preflight.py format-sessions '[{"agent":"BlueLake","track":"test","beads_claimed":["bd-1"],"last_heartbeat":"2025-01-01T10:00:00Z"}]'

# Find stale sessions (15 min threshold)
python skills/orchestrator/scripts/session_cleanup.py find-stale '[{"session_id":"BlueLake-123","agent":"BlueLake","last_heartbeat":"2025-01-01T08:00:00Z"}]' --threshold 15

# Generate session ID
python skills/orchestrator/scripts/session_identity.py generate BlueLake

# Parse session ID
python skills/orchestrator/scripts/session_identity.py parse BlueLake-1735689600
```

## Cleanup

After testing:

```bash
# Release any held reservations
bd list --status=in_progress

# Reset test beads
bd update <bead-id> --status new
```

# Preflight Protocol Reference

Multi-session awareness protocol for orchestrator and conductor commands.

## Overview

Preflight (Phase 0) runs before `/conductor-implement` or `/conductor-orchestrate` to detect concurrent sessions and prevent conflicts.

## Phase 0: 4-Step Protocol

```
┌────────────────────────────────────────────────────────────────┐
│  PHASE 0: PREFLIGHT                                            │
├────────────────────────────────────────────────────────────────┤
│  Step 1: IDENTITY   - Register agent, generate session ID      │
│  Step 2: DETECT     - Scan for active sessions via Agent Mail  │
│  Step 3: DISPLAY    - Show session status, prompt if conflict  │
│  Step 4: PROCEED    - Continue or abort based on user choice   │
└────────────────────────────────────────────────────────────────┘
```

### Step 1: Identity

Register agent identity and generate session ID:

```bash
# Generate unique session identity
timestamp=$(date +%s)
session_id="${BASE_AGENT}-${timestamp}"
display_name="${BASE_AGENT} (session $(date +%H:%M))"

# Register with Agent Mail
toolboxes/agent-mail/agent-mail.js register-agent \
    --project-key "$PROJECT_PATH" \
    --name "$session_id" \
    --program amp \
    --model "$MODEL" \
    --task-description "Session started at $(date +%H:%M)"
```

### Step 2: Detect

Scan for active sessions:

```bash
# Fetch inbox to discover other sessions (2-hour lookback)
since_ts=$(date -u -v-2H +"%Y-%m-%dT%H:%M:%SZ")  # macOS
# since_ts=$(date -u -d "2 hours ago" +"%Y-%m-%dT%H:%M:%SZ")  # Linux

toolboxes/agent-mail/agent-mail.js fetch-inbox \
    --project-key "$PROJECT_PATH" \
    --agent-name "$session_id" \
    --since-ts "$since_ts" \
    --include-bodies true

# Filter for session announcements in output:
# - Look for "[SESSION START]" in subject
# - Parse session_info and check if not stale
# - Build active_sessions list
```

### Step 3: Display

Show session status using display formats (see below).

### Step 4: Proceed

Based on conflict detection:
- **No conflicts**: Proceed to Phase 1
- **Track conflict**: Prompt with [P]roceed/[S]witch/[W]ait
- **Stale session**: Prompt with [T]ake/[W]ait/[I]gnore

## Trigger Conditions

| Command | Preflight | Notes |
|---------|-----------|-------|
| `/conductor-implement` | ✅ Yes | Always runs Phase 0 |
| `/conductor-orchestrate` | ✅ Yes | Always runs Phase 0 |
| `ds` | ❌ Skip | Design sessions don't conflict |
| `bd ready` | ❌ Skip | Read-only command |
| `bd show` | ❌ Skip | Read-only command |
| `bd list` | ❌ Skip | Read-only command |

## Skip Rules

Preflight is skipped when:
1. Command is read-only (`bd ready`, `bd show`, `bd list`)
2. Command is design phase (`ds`, `/conductor-design`)
3. Explicit `--skip-preflight` flag passed

**Note:** If Agent Mail is unavailable during preflight, orchestrator HALTs (does not degrade).

## Agent Mail Integration

### Registration

```bash
# Full registration with task context
toolboxes/agent-mail/agent-mail.js register-agent \
    --project-key "$PROJECT_PATH" \
    --name "$session_id" \
    --program amp \
    --model claude-opus-4-5@20251101 \
    --task-description "Track: $track_id, Started: $timestamp"
```

### Inbox Fetch

```bash
# Fetch with 2-hour lookback for session detection
toolboxes/agent-mail/agent-mail.js fetch-inbox \
    --project-key "$PROJECT_PATH" \
    --agent-name "$session_id" \
    --since-ts "2025-01-01T08:00:00Z" \
    --include-bodies true \
    --limit 50
```

## Error Handling

### Timeout Behavior

Agent Mail operations timeout after 3 seconds:

```bash
# Timeout handling is built into the CLI toolbox
# If timeout occurs, exit with error

toolboxes/agent-mail/agent-mail.js register-agent \
    --project-key "$PROJECT_PATH" \
    --name "$session_id" \
    --program amp \
    --model "$MODEL"

if [ $? -ne 0 ]; then
    # HALT - Agent Mail is required
    echo "❌ Cannot proceed: Agent Mail timeout"
    exit 1
fi
```

### Unavailable Service

```bash
# Check Agent Mail availability via health-check
toolboxes/agent-mail/agent-mail.js health-check --reason "Preflight check"

if [ $? -ne 0 ]; then
    echo "❌ Cannot proceed: Agent Mail required for orchestration"
    # HALT - do not proceed without Agent Mail
    exit 1
fi
```

## Display Formats

### Active Sessions Box

```
┌─ ACTIVE SESSIONS ──────────────────────────────────────────┐
│                                                            │
│ 🟢 BlueLake (session 10:30) - active                       │
│    Track: cc-v2-integration                                │
│    Beads: bd-101 (in_progress)                             │
│    Files: src/api/**                                       │
│    Last seen: 2 min ago                                    │
│                                                            │
│ 🟡 GreenCastle (session 09:15) - stale (12 min)            │
│    Track: doc-updates                                      │
│    Beads: bd-201 (in_progress)                             │
│    Files: docs/**                                          │
│    Last seen: 12 min ago                                   │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Conflict Prompt

```
┌─ CONFLICTS DETECTED ───────────────────────────────────────┐
│ ⚠️  TRACK CONFLICT                                         │
│     BlueLake (session 10:30) is already on cc-v2-integration│
│                                                            │
│ Options:                                                   │
│ [P]roceed anyway - work on different files/beads           │
│ [S]witch track - pick a different track                    │
│ [W]ait - let other session finish first                    │
└────────────────────────────────────────────────────────────┘
```

### Stale Takeover Prompt

```
┌─ STALE SESSION DETECTED ───────────────────────────────────┐
│ GreenCastle (session 09:15) inactive for 12 minutes        │
│                                                            │
│ Reserved files: skills/orchestrator/**                     │
│ Claimed beads: bd-201 (in_progress)                        │
│                                                            │
│ ⚠️  Warning: May have uncommitted work                      │
│                                                            │
│ [T]ake over - release reservations, reset beads to open    │
│ [W]ait - check again in 5 min                              │
│ [I]gnore - proceed without their files/beads               │
└────────────────────────────────────────────────────────────┘
```

## Session Status Indicators

| Icon | Status | Meaning |
|------|--------|---------|
| 🟢 | Active | Heartbeat within 5 min |
| 🟡 | Stale | No heartbeat for 5-10 min |
| 🔴 | Inactive | No heartbeat for >10 min |
| ⚪ | Unknown | No session data available |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `preflight_timeout` | 3s | Max time for Agent Mail operations |
| `session_lookback` | 2h | How far back to scan for sessions |
| `stale_threshold` | 10 min | When to consider session stale |
| `heartbeat_interval` | 5 min | Expected heartbeat frequency |

## References

- [session-identity.md](session-identity.md) - Identity format and collision handling
- [patterns/session-lifecycle.md](patterns/session-lifecycle.md) - Full lifecycle with multi-session
- [agent-coordination.md](agent-coordination.md) - Agent Mail coordination patterns

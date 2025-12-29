# Idle Detection

Reference for automatic handoff prompts after periods of inactivity.

## Overview

Idle detection prompts users to create a handoff when resuming work after a gap, preventing context loss between sessions.

## Mechanism

```
User sends message
       │
       ▼
┌─────────────────────────┐
│ Check .last_activity    │
└─────────────────────────┘
       │
       ▼
┌─────────────────────────┐
│ Gap > threshold?        │──── No ───► Process message
└─────────────────────────┘
       │ Yes
       ▼
┌─────────────────────────┐
│ Prompt: Create handoff? │
└─────────────────────────┘
       │
       ├── Y ──► Create handoff → Process message
       ├── n ──► Process message (no handoff)
       └── skip ► Skip for session → Process message
```

## Activity Marker File

### Location

```
conductor/.last_activity
```

### Behavior

**Touch on significant actions:**
- After `/create_handoff`
- After `/resume_handoff`
- After epic completion (`bd close`)
- After any file edit
- After running tests
- After git operations

**Check on user message:**
- Read mtime of `.last_activity`
- Compare to current time
- If gap > threshold → prompt

### Implementation

```bash
# Touch activity marker
touch_activity() {
  touch conductor/.last_activity
}

# Check for idle gap
check_idle() {
  local marker="conductor/.last_activity"
  local threshold_minutes=${1:-30}
  
  if [ ! -f "$marker" ]; then
    # First activity - create marker, no prompt
    touch "$marker"
    return 1  # Not idle
  fi
  
  local now=$(date +%s)
  local last=$(stat -f %m "$marker" 2>/dev/null || stat -c %Y "$marker")
  local gap_minutes=$(( (now - last) / 60 ))
  
  if [ $gap_minutes -gt $threshold_minutes ]; then
    return 0  # Is idle
  else
    return 1  # Not idle
  fi
}
```

## Prompt Format

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
⏰ It's been 45 minutes since your last activity.

Create a handoff to preserve context?

[Y] Yes - Create handoff first (recommended)
[n] No  - Skip this time
[s] Skip - Don't ask again this session
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## User Options

| Option | Action | Session State |
|--------|--------|---------------|
| **Y** (default) | Create handoff with trigger `idle`, then process message | Update `.last_activity` |
| **n** | Process message immediately, no handoff | Update `.last_activity` |
| **s** (skip) | Process message, disable prompts for session | Set `idle_skip=true` |

### Session Skip State

When user chooses `skip`:

```bash
# In-memory flag (resets on new session)
export HANDOFF_IDLE_SKIP=true
```

The skip state is session-scoped and resets when the session ends.

## Configuration

### Default Threshold

```
30 minutes
```

### Configurable in workflow.md

```yaml
# conductor/workflow.md

handoff:
  idle_threshold_minutes: 30    # Default: 30
  idle_prompt_enabled: true     # Default: true
```

### Override Examples

```yaml
# Quick work - shorter threshold
handoff:
  idle_threshold_minutes: 15

# Long focus sessions - longer threshold  
handoff:
  idle_threshold_minutes: 60

# Disable idle detection entirely
handoff:
  idle_prompt_enabled: false
```

## Integration Point

### Location: maestro-core

Idle detection runs at the **start of message processing**, before any other skill:

```markdown
# In maestro-core SKILL.md

## Session Lifecycle

On user message:

1. **Idle Detection Check**
   - IF `conductor/.last_activity` exists
   - AND mtime > `idle_threshold_minutes` ago
   - AND `idle_skip` not set
   - THEN prompt for handoff

2. Continue with normal routing...
```

### Why maestro-core?

- Runs before any other skill
- Works outside Conductor workflows
- Universal coverage for all sessions
- Single integration point

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No `.last_activity` file | Create file, no prompt (first activity) |
| No `conductor/` directory | Skip idle detection entirely |
| User in `/conductor-implement` | Skip (workflow handles handoffs) |
| User just ran `/resume_handoff` | Skip (already loading context) |
| Threshold set to 0 | Disable idle detection |
| Very long gap (>24h) | Show gap in hours/days |

## Gap Display Format

```bash
format_gap() {
  local minutes=$1
  
  if [ $minutes -lt 60 ]; then
    echo "${minutes} minutes"
  elif [ $minutes -lt 1440 ]; then
    local hours=$((minutes / 60))
    echo "${hours} hour(s)"
  else
    local days=$((minutes / 1440))
    echo "${days} day(s)"
  fi
}
```

Examples:
- 45 minutes → "45 minutes"
- 120 minutes → "2 hour(s)"
- 2880 minutes → "2 day(s)"

## Handoff Content for Idle Trigger

When creating handoff with `idle` trigger:

```markdown
---
timestamp: 2025-12-29T15:30:00.123+07:00
trigger: idle
track_id: auth-system_20251229
git_commit: abc123f
git_branch: feat/auth-system
author: agent
---

# Handoff: auth-system_20251229 | idle

## Context

Session interrupted after 45 minutes of inactivity.
Last activity: Working on E2 login endpoint.

## Changes

- `src/auth/login.ts:50-80` - Partial login handler
- `tests/auth/login.test.ts:1-30` - Started test file

## Learnings

- Login flow requires session management
- Redis connection pool needs configuration

## Next Steps

1. [ ] Complete login handler implementation
2. [ ] Finish test cases
3. [ ] Wire up session middleware
```

## Quiet Mode

When `handoff.quiet: true` in workflow.md:

```yaml
handoff:
  quiet: true
```

Behavior:
- Still check for idle gap
- Still create handoff automatically
- **Don't show prompt** - just log creation
- Show brief confirmation: "📝 Auto-handoff created (idle)"

## Testing

### Manual Test

```bash
# Set up test scenario
touch -t 202512291000 conductor/.last_activity  # 10:00 AM
# Send message at 10:45 AM (45 min gap)
# Should prompt for handoff
```

### Verify Prompt

1. Create `.last_activity` with old timestamp
2. Send any message
3. Verify prompt appears
4. Choose Y → verify handoff created
5. Choose n → verify no handoff
6. Choose s → verify no more prompts

## Metrics (Optional)

Track idle detection events:

```jsonl
{"event": "idle_detected", "gap_minutes": 45, "action": "handoff", "timestamp": "..."}
{"event": "idle_detected", "gap_minutes": 30, "action": "skip", "timestamp": "..."}
{"event": "idle_detected", "gap_minutes": 120, "action": "session_skip", "timestamp": "..."}
```

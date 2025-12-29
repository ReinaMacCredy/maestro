# Handoff Triggers

Reference for the 6 automatic handoff triggers.

## Trigger Overview

| Trigger | Integration Point | Frequency | Automatic |
|---------|-------------------|-----------|-----------|
| `design-end` | After `/conductor-newtrack` completes | 1x per track | ✅ |
| `epic-start` | Before each epic in `/conductor-implement` | N per track | ✅ |
| `epic-end` | After each epic closes | N per track | ✅ |
| `pre-finish` | Start of `/conductor-finish` | 1x per track | ✅ |
| `manual` | User runs `/create_handoff` | On-demand | ❌ |
| `idle` | Message after 30min gap | On-demand | ✅ (prompted) |

## Trigger Details

### `design-end`

**When:** After `/conductor-newtrack` successfully creates spec.md and plan.md.

**Why:** Captures design decisions and rationale before implementation begins.

**Integration:**

```markdown
# In references/workflows/newtrack.md, Phase 7

## Phase 7: Post-Track Handoff

After successful track creation:

1. Create handoff with trigger: `design-end`
2. Include:
   - Key design decisions from design.md
   - Approach rationale
   - Constraints identified
   - Next steps: "Start implementation with `bd ready`"
```

**Content Focus:**
- Design decisions made
- Alternatives considered and rejected
- Constraints discovered
- Architecture approach

---

### `epic-start`

**When:** Before starting each epic in `/conductor-implement`.

**Why:** Captures context before potentially long implementation session.

**Integration:**

```markdown
# In references/workflows/implement.md

## Before Each Epic

1. Load most recent handoff for track
2. Create handoff with trigger: `epic-start`
3. Include:
   - Epic scope from plan.md
   - Dependencies from beads
   - Entry conditions
   - Expected outcomes
```

**Content Focus:**
- Epic scope and goals
- Prior context loaded
- Dependencies and blockers
- Expected deliverables

---

### `epic-end`

**When:** After an epic is closed (completed, skipped, or blocked).

**Why:** Captures learnings and state for next session.

**Integration:**

```markdown
# In references/workflows/implement.md

## After Epic Close

1. Create handoff with trigger: `epic-end`
2. Include:
   - Work completed in epic
   - Files changed
   - Learnings and gotchas
   - Close reason (completed/skipped/blocked)
   - Next epic to tackle
```

**Content Focus:**
- Work accomplished
- Files modified (git diff)
- Learnings discovered
- Blockers encountered
- Next steps

---

### `pre-finish`

**When:** At the start of `/conductor-finish`, before any archive operations.

**Why:** Final snapshot of track state before archival.

**Integration:**

```markdown
# In references/finish-workflow.md, Phase 0

## Phase 0: Pre-Finish Handoff

Before starting finish workflow:

1. Load most recent handoff
2. Create handoff with trigger: `pre-finish`
3. Include:
   - Track completion summary
   - All changes made
   - Final learnings
   - Post-track TODOs (if any)
```

**Content Focus:**
- Track summary
- Total changes (all epics)
- Consolidated learnings
- Follow-up work needed
- Verification status

---

### `manual`

**When:** User explicitly runs `/create_handoff`.

**Why:** User-initiated context capture.

**Integration:**

```markdown
# Direct command, no automatic trigger

/create_handoff
```

**Content Focus:**
- Current work state
- Recent changes
- Immediate context
- User-defined next steps

---

### `idle`

**When:** User sends a message after 30+ minutes of inactivity.

**Why:** Prompts context capture before potential session end.

**Integration:**

```markdown
# In maestro-core session lifecycle

On user message:
  IF conductor/.last_activity exists
    AND mtime > 30 minutes ago
  THEN prompt:
    "It's been a while. Create handoff first? [Y/n/skip]"
```

**User Options:**
- **Y (default)**: Create handoff, then process message
- **n**: Skip handoff, process message
- **skip**: Skip handoff for this session (don't ask again)

**Content Focus:**
- Session summary
- Work interrupted
- Context that would be lost
- Resume instructions

See [idle-detection.md](idle-detection.md) for implementation details.

## Trigger Configuration

In `conductor/workflow.md`:

```yaml
handoff:
  # Suppress handoff prompts (still creates files)
  quiet: false
  
  # Idle detection threshold
  idle_threshold_minutes: 30
  
  # Auto-triggers (disable specific ones)
  auto_triggers:
    design-end: true      # Always recommended
    epic-start: true      # Disable for short epics
    epic-end: true        # Disable for short epics
    pre-finish: true      # Always recommended
```

### Disabling Triggers

```yaml
handoff:
  auto_triggers:
    epic-start: false     # Skip epic-start handoffs
    epic-end: false       # Skip epic-end handoffs
```

**Note:** `design-end` and `pre-finish` cannot be disabled (critical for context preservation).

## Trigger Priority

When multiple triggers could apply:

```
pre-finish > epic-end > epic-start > design-end > idle > manual
```

Only one trigger fires per event.

## Validation Integration

Each trigger captures validation state:

```yaml
validation_snapshot:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
  retries: 0
  last_failure: null
```

This enables audit trail of validation progress.

## Examples

### Design-End Handoff

```markdown
---
timestamp: 2025-12-29T10:00:00.123+07:00
trigger: design-end
track_id: auth-system_20251229
git_commit: abc123f
git_branch: feat/auth-system
author: agent
validation_snapshot:
  gates_passed: [design]
  current_gate: spec
---

# Handoff: auth-system_20251229 | design-end

## Context

Completed design session for JWT authentication system.
- Approach: RS256 with rotating keys
- Storage: Redis for revocation list
- Token lifetime: 15min access, 7d refresh

## Changes

- `conductor/tracks/auth-system_20251229/design.md` - Created
- `conductor/tracks/auth-system_20251229/spec.md` - Created  
- `conductor/tracks/auth-system_20251229/plan.md` - Created

## Learnings

- RS256 chosen over HS256 for key rotation support
- Redis revocation preferred over database for performance
- Need rate limiting on refresh endpoint

## Next Steps

1. [ ] Run `bd ready` to see available tasks
2. [ ] Start with E1: Core JWT module
3. [ ] Set up Redis connection first
```

### Epic-End Handoff

```markdown
---
timestamp: 2025-12-29T15:30:00.456+07:00
trigger: epic-end
track_id: auth-system_20251229
bead_id: E1-jwt-core
git_commit: def456a
git_branch: feat/auth-system
author: agent
validation_snapshot:
  gates_passed: [design, spec, plan-structure, plan-execution]
  current_gate: completion
---

# Handoff: auth-system_20251229 | epic-end

## Context

Completed E1: Core JWT module.
- All tests passing (12 new tests)
- Coverage: 94%
- Bead status: completed

## Changes

- `src/auth/jwt.ts:1-120` - JWT token generation and validation
- `src/auth/keys.ts:1-45` - Key pair management
- `tests/auth/jwt.test.ts:1-200` - Unit tests

## Learnings

- jsonwebtoken library needs explicit algorithm specification
- Key rotation requires graceful fallback to old keys
- Redis TTL should match token lifetime

## Next Steps

1. [ ] Start E2: Login endpoint
2. [ ] Wire JWT module to login handler
3. [ ] Add integration tests
```

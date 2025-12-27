# LEDGER.md Format

The LEDGER.md file contains live session state that survives `/clear` and compaction.

## Location

`conductor/sessions/active/LEDGER.md`

## Git Status

**Gitignored** - This is personal session state, not shared.

## Required Fields

```yaml
---
updated: 2025-12-27T10:30:00Z
session_id: T-abc123
platform: claude | amp | codex
---
```

## Conductor Session Fields (Optional)

When working with Conductor-managed tracks, these fields store session state:

```yaml
---
updated: 2025-12-27T10:30:00Z
session_id: T-abc123
platform: claude
bound_track: feature-name_20251227
bound_bead: my-workflow:3-xyz
mode: SA
tdd_phase: GREEN
heartbeat: 2025-12-27T10:25:00Z
---
```

| Field | Type | Description |
|-------|------|-------------|
| `bound_track` | string \| null | Currently active track ID |
| `bound_bead` | string \| null | Currently claimed bead/task ID |
| `mode` | `SA` \| `MA` | Session mode (Single-Agent or Multi-Agent) |
| `tdd_phase` | `RED` \| `GREEN` \| `REFACTOR` \| null | Current TDD phase if using --tdd |
| `heartbeat` | ISO 8601 | Last activity timestamp for session liveness |

**Mode locking:** Once set at session start, `mode` should not change mid-session.

**Heartbeat protocol:** Updated every 5 minutes during active work. Sessions with heartbeat >10 min old are considered stale.

## Full Template

```markdown
---
updated: 2025-12-27T10:30:00Z
session_id: T-abc123
platform: claude
bound_track: feature-name_20251227
bound_bead: my-workflow:3-xyz
mode: SA
tdd_phase: null
heartbeat: 2025-12-27T10:25:00Z
---

# Session Ledger

## Active Task

bead: my-workflow:3-xyz
track: feature-name_20251227

## Goal

What we're trying to accomplish in this session.

## State

### Done
- Completed item 1
- Completed item 2

### Now
- Current work item

### Next
- Upcoming item 1
- Upcoming item 2

## Key Decisions

| Decision | Reasoning | When |
|----------|-----------|------|
| Chose X over Y | Because Z | 10:15 |

## Working Set

**Branch:** feature/my-feature

**Modified:**
- `path/to/file1.ts`
- `path/to/file2.ts`

**Read:**
- `path/to/reference.md` - For understanding pattern

## Open Questions

- Unresolved question 1?
- Unresolved question 2?
```

## Section Details

### Active Task

References the current bead and track:

```markdown
## Active Task

bead: my-workflow:3-xyz
track: feature-name_20251227
```

### Goal

1-2 sentences describing session objective:

```markdown
## Goal

Implement the authentication system with JWT tokens and refresh logic.
```

### State (Done/Now/Next)

Current progress tracking:

```markdown
## State

### Done
- Created auth middleware
- Added JWT validation

### Now
- Implementing refresh token logic

### Next
- Add logout endpoint
- Write integration tests
```

### Key Decisions

Choices made with reasoning:

```markdown
## Key Decisions

| Decision | Reasoning | When |
|----------|-----------|------|
| Use RS256 over HS256 | Key rotation support | 10:15 |
| Store refresh in httpOnly cookie | XSS protection | 10:30 |
```

### Working Set

Branch and modified files:

```markdown
## Working Set

**Branch:** feature/auth-jwt

**Modified:**
- `src/middleware/auth.ts`
- `src/utils/jwt.ts`

**Read:**
- `docs/security.md` - Security requirements
```

### Open Questions

Unresolved items:

```markdown
## Open Questions

- Should we use sliding window for refresh tokens?
- What's the token expiry for mobile clients?
```

## Staleness

A ledger is considered stale after 24 hours without update.

Stale ledgers are automatically archived on next session start.

## Updates

The `updated` field MUST be set to current ISO 8601 timestamp on every save.

## Track Binding

When working on a Conductor track, the LEDGER binds to that track:

**Binding occurs when:**
- `/conductor-implement <track>` starts
- `bd update <id> --status in_progress` claims a task

**Binding clears when:**
- Track completes (`/conductor-finish`)
- Session ends gracefully
- Session is archived (stale or manual)

**Track switch behavior:**
- If `bound_track` differs from new track, auto-archive current LEDGER before binding new track
- This preserves context from previous track in `conductor/sessions/archive/`

**Example track binding flow:**

```yaml
# Before claim
bound_track: null
bound_bead: null

# After: /conductor-implement auth_20251227
bound_track: auth_20251227
bound_bead: null

# After: bd update my-workflow:3-xyz --status in_progress
bound_track: auth_20251227
bound_bead: my-workflow:3-xyz

# After: bd close my-workflow:3-xyz --reason completed
bound_track: auth_20251227
bound_bead: null

# After: /conductor-finish
bound_track: null
bound_bead: null
```

## Continuity-Conductor Chain

The LEDGER.md serves as the integration point between Continuity and Conductor:

| Conductor Phase | Continuity Action |
|-----------------|-------------------|
| `/conductor-implement` Phase 0.5 | Load LEDGER, check binding, archive if switching |
| `/conductor-finish` Phase 6.5 | Create handoff, clear bindings, optionally delete |
| `ds` (Design Session) Init | Load LEDGER for prior context display |

**Auto-archive on track switch:**
When `/conductor-implement` detects `bound_track` differs from the new track, it runs `continuity handoff track-switch` before binding to the new track.

**Non-blocking guarantee:**
All continuity operations in Conductor phases are non-blocking. Failures log warnings but never halt Conductor commands.

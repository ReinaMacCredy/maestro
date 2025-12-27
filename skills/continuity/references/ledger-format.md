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

## Full Template

```markdown
---
updated: 2025-12-27T10:30:00Z
session_id: T-abc123
platform: claude
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

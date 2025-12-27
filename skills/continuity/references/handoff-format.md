# Handoff Format

Handoffs are archived session states for cross-session context transfer.

## Location

`conductor/sessions/archive/YYYY-MM-DD-HH-MM-<trigger>.md`

## Git Status

**Committed** - Handoffs are shared history.

## Filename Format

```text
2025-12-27-10-30-manual.md
2025-12-27-14-45-pre-compact.md
2025-12-27-18-00-session-end.md
2025-12-27-10-00-stale.md
```

## Required Frontmatter

```yaml
---
date: 2025-12-27T10:30:00Z
session_id: T-abc123
trigger: manual | pre-compact | session-end | stale
status: complete | interrupted | handoff
---
```

## Full Template

```markdown
---
date: 2025-12-27T10:30:00Z
session_id: T-abc123
trigger: manual
status: complete
---

# Session Handoff

## Summary

Brief 1-2 sentence summary of what was accomplished.

## Completed

- Task 1 completed
- Task 2 completed

## In Progress

- Task being worked on when session ended

## Blocked

- Blocked item and reason
- Or "None"

## Next Steps

1. First thing next session should do
2. Second thing

## Key Decisions

| Decision | Reasoning |
|----------|-----------|
| Chose X | Because Y |

## Artifacts

### Modified
| File | Change |
|------|--------|
| `path/to/file.ts` | Description |

### Created
| File | Purpose |
|------|---------|
| `path/to/new.ts` | Purpose |

## Context for Next Session

Any important context the next session needs to know.

## Open Questions

- Questions that remain unresolved
```

## Trigger Types

### manual

User explicitly ran `continuity handoff`:

```yaml
trigger: manual
status: complete
```

### pre-compact

Auto-triggered before compaction:

```yaml
trigger: pre-compact
status: handoff
```

### session-end

Auto-triggered on clean session exit:

```yaml
trigger: session-end
status: complete
```

### stale

Auto-triggered when loading stale ledger (>24h):

```yaml
trigger: stale
status: interrupted
```

## Status Types

### complete

Session ended normally, all work done:

```yaml
status: complete
```

### interrupted

Session ended unexpectedly or ledger was stale:

```yaml
status: interrupted
```

### handoff

Session ended for handoff to new session:

```yaml
status: handoff
```

## Section Details

### Summary

One-liner of what happened:

```markdown
## Summary

Completed JWT authentication middleware and started refresh token logic.
```

### Completed

What got done:

```markdown
## Completed

- Implemented JWT validation middleware
- Added token generation utility
- Created auth configuration
```

### In Progress

What was being worked on:

```markdown
## In Progress

- Refresh token rotation logic (50% complete)
```

### Blocked

Blockers encountered:

```markdown
## Blocked

- Waiting for security team review of token expiry
```

### Next Steps

Actionable items for next session:

```markdown
## Next Steps

1. Complete refresh token rotation
2. Add logout endpoint
3. Write integration tests
```

### Context for Next Session

Important context that might be lost:

```markdown
## Context for Next Session

The auth middleware uses RS256 because we need key rotation support.
Token expiry is 15 minutes for access, 7 days for refresh.
See `docs/security.md` for full requirements.
```

## Indexing

Handoffs are indexed by `scripts/artifact-index.py` for full-text search.

The index is stored at `conductor/.cache/artifact-index.db`.

## Cleanup

Handoffs older than 30 days can be cleaned up with:

```bash
uv run scripts/artifact-cleanup.py --max-age 30
```

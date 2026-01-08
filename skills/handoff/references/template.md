# Handoff File Template

## Filename Format

```
YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
```

Example: `2026-01-04_10-30-00-123_auth-system_manual.md`

## Template

```markdown
---
track_id: <track_id>
trigger: <manual|design-end|epic-start|epic-end|pre-finish|idle>
timestamp: <ISO-8601>
git_branch: <branch>
git_commit: <short-sha>
bead_id: <epic-id or null>
section: <N>
progress_percent: <0-100>
---

# Handoff: <track_id>

## Context

<Current state summary. What was being worked on.>

## Changes

<What was accomplished in this session.>

- [ ] Completed task 1
- [ ] Completed task 2

## Learnings

<Key decisions, gotchas, insights.>

## Next Steps

<What the next session should do.>

1. [ ] First priority task
2. [ ] Second priority task
3. [ ] Third priority task
```

## Example

```markdown
---
track_id: auth-system_20251229
trigger: manual
timestamp: 2026-01-04T10:30:00.123Z
git_branch: feat/auth-system
git_commit: abc123f
bead_id: my-workflow:3-lzks
section: 3
progress_percent: 45
---

# Handoff: auth-system_20251229

## Context

Working on JWT authentication module. E1 (Token generation) complete.
Currently implementing E2 (Login endpoint).

## Changes

- Implemented JWT token generation with RS256
- Added token validation middleware
- Created tests for token expiry edge cases

## Learnings

- RS256 requires public/private key pair - stored in env vars
- Token refresh needs to be stateless for horizontal scaling
- Found bug in auth.go line 145 - off-by-one in expiry calc

## Next Steps

1. [ ] Fix expiry bug in auth.go:145
2. [ ] Complete login endpoint handler
3. [ ] Wire JWT module to login handler
4. [ ] Add rate limiting to auth routes
```

## Index Template

Each track has an `index.md`:

```markdown
---
track_id: <track_id>
created: <ISO-8601>
last_updated: <ISO-8601>
---

# Handoff Log: <track_id>

| Timestamp | Trigger | Bead | Summary | File |
|-----------|---------|------|---------|------|
| 2026-01-04 10:30 | manual | lzks | E1 complete | [→](./2026-01-04_10-30-00_auth-system_manual.md) |
```

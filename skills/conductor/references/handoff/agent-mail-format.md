# Agent Mail Handoff Format

Reference for handoff messages sent via Agent Mail MCP.

## Overview

Agent Mail provides the **primary storage** for handoffs, with markdown files as secondary (for git history). This enables:

- **FTS5 search** via `search_messages()` across all handoffs
- **Thread summaries** via `summarize_thread()` for context loading
- **Cross-session continuity** without file system dependencies
- **Multi-agent coordination** when orchestrating parallel work

## Message Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `project_key` | string | Absolute workspace path (e.g., `/Users/alice/project`) |
| `sender_name` | string | Agent identity (adjective+noun, e.g., "BlueLake") |
| `to` | string[] | Recipients (usually `["Human"]` or orchestrator name) |
| `subject` | string | Trigger prefix + context (see Subject Format) |
| `body_md` | string | Structured markdown (see Body Format) |
| `thread_id` | string | Track-based thread ID (see Thread Structure) |
| `importance` | string | `"normal"` for manual, `"high"` for auto triggers |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `ack_required` | boolean | `true` for blocking handoffs (rare) |
| `cc` | string[] | Additional recipients for visibility |

## Thread Structure

### Thread ID Conventions

| Scope | Thread ID Format | Example |
|-------|------------------|---------|
| Track-specific | `handoff-<track_id>` | `handoff-auth-system_20251229` |
| General (non-track) | `handoff-general` | `handoff-general` |
| Epic-specific | `handoff-<track_id>-<epic_id>` | `handoff-auth-system_20251229-E1` |

### Thread Hierarchy

```
handoff-general
├── Non-track work
└── Cross-cutting concerns

handoff-<track_id>
├── design-end messages
├── epic-start/epic-end messages
├── pre-finish messages
└── manual handoffs for track

handoff-<track_id>-<epic_id>
├── Epic-specific detailed context
└── Fine-grained checkpoints
```

## Subject Format

Subject lines use a prefix pattern for filtering:

```
[HANDOFF:<trigger>] <track_id> - <brief_context>
```

### Subject Examples by Trigger

| Trigger | Subject Example |
|---------|-----------------|
| `design-end` | `[HANDOFF:design-end] auth-system - JWT design complete` |
| `epic-start` | `[HANDOFF:epic-start] auth-system/E1 - Starting JWT core` |
| `epic-end` | `[HANDOFF:epic-end] auth-system/E1 - JWT core complete` |
| `pre-finish` | `[HANDOFF:pre-finish] auth-system - Ready for archive` |
| `manual` | `[HANDOFF:manual] auth-system - Mid-session checkpoint` |
| `idle` | `[HANDOFF:idle] auth-system - Session gap detected` |

### Search Queries

Subject prefixes enable targeted search:

```python
# Find all handoffs
search_messages(project_key, query="HANDOFF")

# Find specific trigger type
search_messages(project_key, query="HANDOFF:design-end")

# Find track-specific handoffs
search_messages(project_key, query="HANDOFF AND auth-system")
```

## Body Format

### Structure

The body uses four sections matching the markdown template:

```markdown
## Context

{Current work state, active decisions, phase}

- **Track:** <track_id>
- **Trigger:** <trigger_type>
- **Phase:** <current_phase>
- **Git:** <branch>@<commit_sha>

## Changes

{Files modified with line references}

- `path/to/file.ts:10-45` - Description
- `path/to/other.ts:100-120` - Description

## Learnings

{Patterns discovered, gotchas, important context}

- **Pattern:** Description
- **Gotcha:** Something to watch out for
- **Decision:** Why we chose X over Y

## Next Steps

{Immediate actions for resuming agent}

1. [ ] First task
2. [ ] Second task
3. [ ] Verification step
```

### Validation Snapshot (Optional)

Append validation state when relevant:

```markdown
---

**Validation:** gates_passed=[design, spec], current_gate=plan-structure
```

## Importance Levels

| Trigger | Importance | Rationale |
|---------|------------|-----------|
| `design-end` | `high` | Critical checkpoint after design |
| `epic-start` | `high` | Critical before long implementation |
| `epic-end` | `high` | Critical for continuity |
| `pre-finish` | `high` | Critical before archive |
| `manual` | `normal` | User-initiated, not urgent |
| `idle` | `normal` | Advisory, not blocking |

## Example Messages

### design-end

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:design-end] auth-system_20251229 - JWT design complete",
    body_md="""## Context

Completed design session for JWT authentication system.

- **Track:** auth-system_20251229
- **Trigger:** design-end
- **Phase:** Design → Implementation
- **Git:** feat/auth-system@abc123f

Key decisions:
- RS256 with rotating keys
- Redis for revocation list
- 15min access / 7d refresh tokens

## Changes

- `conductor/tracks/auth-system_20251229/design.md` - Created
- `conductor/tracks/auth-system_20251229/spec.md` - Created
- `conductor/tracks/auth-system_20251229/plan.md` - Created

## Learnings

- **Decision:** RS256 over HS256 for key rotation support
- **Decision:** Redis revocation over DB for performance
- **Gotcha:** Need rate limiting on refresh endpoint

## Next Steps

1. [ ] Run `bd ready` to see available tasks
2. [ ] Start with E1: Core JWT module
3. [ ] Set up Redis connection first
""",
    thread_id="handoff-auth-system_20251229",
    importance="high"
)
```

### epic-start

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:epic-start] auth-system_20251229/E1 - Starting JWT core",
    body_md="""## Context

Starting E1: Core JWT module implementation.

- **Track:** auth-system_20251229
- **Trigger:** epic-start
- **Phase:** Implementation (Epic 1 of 3)
- **Git:** feat/auth-system@abc123f

Dependencies: None (first epic)
Expected deliverables: jwt.ts, keys.ts, tests

## Changes

No changes yet - starting fresh.

## Learnings

Prior context from design-end:
- RS256 algorithm selected
- Redis for revocation
- 15min/7d token lifetimes

## Next Steps

1. [ ] Create src/auth/jwt.ts with token generation
2. [ ] Create src/auth/keys.ts with key management
3. [ ] Write unit tests first (TDD)
4. [ ] Wire to Redis for revocation
""",
    thread_id="handoff-auth-system_20251229",
    importance="high"
)
```

### epic-end

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:epic-end] auth-system_20251229/E1 - JWT core complete",
    body_md="""## Context

Completed E1: Core JWT module.

- **Track:** auth-system_20251229
- **Trigger:** epic-end
- **Phase:** Implementation (Epic 1 complete, 2 remaining)
- **Git:** feat/auth-system@def456a

All tests passing (12 new tests), coverage 94%.
Bead status: completed

## Changes

- `src/auth/jwt.ts:1-120` - JWT token generation and validation
- `src/auth/keys.ts:1-45` - Key pair management
- `tests/auth/jwt.test.ts:1-200` - Unit tests

## Learnings

- **Gotcha:** jsonwebtoken needs explicit algorithm specification
- **Pattern:** Key rotation requires graceful fallback to old keys
- **Tip:** Redis TTL should match token lifetime

## Next Steps

1. [ ] Start E2: Login endpoint
2. [ ] Wire JWT module to login handler
3. [ ] Add integration tests
""",
    thread_id="handoff-auth-system_20251229",
    importance="high"
)
```

### pre-finish

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:pre-finish] auth-system_20251229 - Ready for archive",
    body_md="""## Context

Track ready for finish workflow.

- **Track:** auth-system_20251229
- **Trigger:** pre-finish
- **Phase:** Completion
- **Git:** feat/auth-system@ghi789b

All epics complete, all beads closed.
Total: 3 epics, 12 tasks, 45 tests added.

## Changes

Summary of all changes:
- `src/auth/` - New authentication module (5 files)
- `tests/auth/` - Test suite (3 files)
- `docs/auth.md` - Documentation

## Learnings

Consolidated from all epics:
- RS256 with Redis revocation is production-ready pattern
- Rate limiting essential for refresh endpoints
- Key rotation needs careful graceful degradation

## Next Steps

1. [ ] Run `/conductor-finish` to archive
2. [ ] Review extracted learnings
3. [ ] Create PR for merge
""",
    thread_id="handoff-auth-system_20251229",
    importance="high"
)
```

### manual

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:manual] auth-system_20251229 - Mid-session checkpoint",
    body_md="""## Context

Manual checkpoint during E2 implementation.

- **Track:** auth-system_20251229
- **Trigger:** manual
- **Phase:** Implementation (E2 in progress)
- **Git:** feat/auth-system@jkl012c

Stopping for the day, E2 partially complete.

## Changes

- `src/auth/login.ts:1-80` - Login handler (partial)
- `tests/auth/login.test.ts:1-50` - Tests (3 of 6 written)

## Learnings

- Password hashing needs bcrypt cost factor tuning
- Login rate limiting should be per-IP, not per-user

## Next Steps

1. [ ] Complete login handler validation
2. [ ] Finish remaining 3 tests
3. [ ] Wire to session management
""",
    thread_id="handoff-auth-system_20251229",
    importance="normal"
)
```

### idle

```python
send_message(
    project_key="/Users/alice/project",
    sender_name="BlueLake",
    to=["Human"],
    subject="[HANDOFF:idle] auth-system_20251229 - Session gap detected",
    body_md="""## Context

30+ minute gap detected, creating checkpoint.

- **Track:** auth-system_20251229
- **Trigger:** idle
- **Phase:** Implementation (E2 in progress)
- **Git:** feat/auth-system@mno345d

Last activity: Working on login validation.

## Changes

- `src/auth/login.ts:60-80` - Added input validation

## Learnings

- Input validation should use zod schema
- Error messages need sanitization (no password hints)

## Next Steps

1. [ ] Continue with login error handling
2. [ ] Add rate limiting middleware
3. [ ] Complete E2 tests
""",
    thread_id="handoff-auth-system_20251229",
    importance="normal"
)
```

## Resume via Agent Mail

To load handoff context via Agent Mail:

```python
# Get thread summary for track
summary = summarize_thread(
    project_key="/Users/alice/project",
    thread_id="handoff-auth-system_20251229",
    llm_mode=True
)

# Or fetch recent messages directly
messages = fetch_inbox(
    project_key="/Users/alice/project",
    agent_name="BlueLake",
    include_bodies=True,
    limit=5
)
```

See [resume.md](resume.md) for the full resume workflow.

## Fallback Behavior

If Agent Mail MCP is unavailable:

1. **On send:** Log warning, proceed with markdown-only
2. **On resume:** Fall back to file-based handoff search

```text
⚠️ Agent Mail unavailable - using file-based handoffs only
```

## Integration with Markdown Files

Agent Mail is **primary**, markdown is **secondary**:

1. Send to Agent Mail first
2. Write markdown file for git history
3. On resume, try Agent Mail first, fall back to markdown

This provides:
- Fast search (FTS5) via Agent Mail
- Git-committed history via markdown
- Works offline via markdown fallback

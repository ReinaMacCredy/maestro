# Summary Protocol

> **Standard format for sub-agent completion reports via Agent Mail.**

## Overview

Sub-agents MUST send a summary message via Agent Mail before returning. This ensures:
- Main thread has visibility into completed work
- Context is preserved across sessions
- Errors are properly reported

## Summary Format

```markdown
## Status
SUCCEEDED | PARTIAL | FAILED

## Files Changed
- path/to/file1.ts (added)
- path/to/file2.ts (modified)
- path/to/file3.ts (deleted)

## Key Decisions
- Decision 1: rationale
- Decision 2: rationale

## Issues (if any)
- Issue description
```

## Status Values

| Status | Meaning | When to Use |
|--------|---------|-------------|
| `SUCCEEDED` | Task fully completed | All acceptance criteria met |
| `PARTIAL` | Task partially completed | Some work done, blockers remain |
| `FAILED` | Task could not be completed | Error or blocker prevented work |

## Agent Mail Message Structure

```python
send_message(
    project_key="/path/to/project",
    sender_name="BlueLake",  # Agent's registered name
    to=["Orchestrator"],     # Main thread's agent name
    subject=f"Completed: {task_id} - {task_summary}",
    body_md=summary_template,
    importance="normal",     # high for failures
    thread_id=epic_thread_id # Links to epic thread
)
```

## Example: Successful Completion

```python
send_message(
    project_key="/Users/dev/myproject",
    sender_name="BlueLake",
    to=["Orchestrator"],
    subject="Completed: bd-101 - Add user authentication",
    body_md="""
## Status
SUCCEEDED

## Files Changed
- src/auth/middleware.ts (added)
- src/routes/login.ts (modified)
- tests/auth.test.ts (added)

## Key Decisions
- Used JWT for tokens: standard, well-supported
- 15min token expiry: balances security/UX

## Issues (if any)
None
""",
    importance="normal",
    thread_id="epic-my-workflow-001"
)
```

## Example: Partial Completion

```python
send_message(
    project_key="/Users/dev/myproject",
    sender_name="GreenCastle",
    to=["Orchestrator"],
    subject="Partial: bd-102 - Database migration",
    body_md="""
## Status
PARTIAL

## Files Changed
- migrations/001_users.sql (added)
- src/db/schema.ts (modified)

## Key Decisions
- Split migration into phases: safer rollback

## Issues (if any)
- Blocked by bd-100 (schema design not finalized)
- Tests skipped pending schema approval
""",
    importance="high",
    thread_id="epic-my-workflow-001"
)
```

## Example: Failed Task

```python
send_message(
    project_key="/Users/dev/myproject",
    sender_name="RedStone",
    to=["Orchestrator"],
    subject="Failed: bd-103 - Deploy to staging",
    body_md="""
## Status
FAILED

## Files Changed
None

## Key Decisions
N/A

## Issues (if any)
- CI pipeline failed: missing DEPLOY_KEY secret
- Cannot proceed without DevOps input
- Marked bead as blocked
""",
    importance="high",
    thread_id="epic-my-workflow-001"
)
```

## Files Changed Format

Use consistent action verbs:

| Action | Meaning |
|--------|---------|
| `added` | New file created |
| `modified` | Existing file changed |
| `deleted` | File removed |
| `renamed` | File moved/renamed |

## Key Decisions Guidelines

Document decisions that:
- Affect architecture or design
- Deviate from initial plan
- Have trade-offs worth noting
- Future developers should understand

## Thread Linking

Always include `thread_id` to link messages to the epic:
- Enables thread summarization
- Groups related work
- Maintains conversation history

## Validation

Before sending, verify:
1. ☐ Status reflects actual outcome
2. ☐ All changed files listed
3. ☐ Important decisions documented
4. ☐ Issues describe blockers with bead IDs
5. ☐ thread_id links to correct epic

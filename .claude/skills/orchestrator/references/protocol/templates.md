# Message Templates

Templates for all 11 message types in the orchestrator protocol.

## Subject Pattern Conventions

| Type | Subject Pattern |
|------|-----------------|
| ASSIGN | `[ASSIGN] Track {track}: {summary}` |
| WAKE | `[WAKE] {reason}` |
| PING | `[PING] Liveness check` |
| PONG | `[PONG] Status: {status}` |
| PROGRESS | `[PROGRESS] {bead_id}: {percent}%` |
| BLOCKED | `[BLOCKED] {bead_id}: {blocker}` |
| COMPLETED | `[TRACK COMPLETE] Track {track}` |
| FAILED | `[FAILED] Track {track}: {bead_id}` |
| STEAL | `[STEAL] Request for {bead_id}` |
| RELEASE | `[RELEASE] {bead_id}` |
| ESCALATE | `[ESCALATE] {issue}` |

---

## ASSIGN

Assigns a track with beads to a worker.

```yaml
---
type: ASSIGN
track: A
beads:
  - my-workflow:3-zyci.1.1
  - my-workflow:3-zyci.1.2
file_scope: "src/api/*"
thread_id: my-workflow:3-zyci
importance: high
---

You are assigned to Track A: API Implementation.

## Tasks
- 1.1.1: Create API router
- 1.1.2: Add validation middleware

## File Scope
Only modify files in `src/api/`.

## Reporting
Send progress updates every 15 minutes.
```

---

## WAKE

Wakes a sleeping worker when a dependency is satisfied.

```yaml
---
type: WAKE
reason: dependency_satisfied
dependency_satisfied: my-workflow:3-zyci.1.1
importance: high
---

Dependency `my-workflow:3-zyci.1.1` has been completed.

You can now proceed with your blocked work on `my-workflow:3-zyci.1.2`.
```

---

## PING

Orchestrator liveness check to workers.

```yaml
---
type: PING
request_id: ping-20260103-143022
importance: normal
---

Liveness check. Please respond with PONG including your current status.
```

---

## PONG

Worker response to PING with current status.

```yaml
---
type: PONG
request_id: ping-20260103-143022
status: working
importance: normal
---

## Current State
- Bead: my-workflow:3-zyci.1.2
- Progress: 60%
- ETA: 10 minutes
```

---

## PROGRESS

Worker progress update (heartbeat).

```yaml
---
type: PROGRESS
bead_id: my-workflow:3-zyci.1.2
percent: 75
notes: "Validation complete, starting tests"
importance: low
---

## Work Completed
- Created validation middleware
- Added error handling

## Next Steps
- Write unit tests
- Integration test with router
```

---

## BLOCKED

Worker reports they cannot proceed.

```yaml
---
type: BLOCKED
bead_id: my-workflow:3-zyci.1.2
blocker: my-workflow:3-zyci.1.1
needs: "Parser module exports"
importance: high
---

## Blocker Details
I need the `parse_message` function from the parser module to proceed.

## Requested Action
Please notify me when `my-workflow:3-zyci.1.1` is complete.

## Alternative
If urgent, I can implement a temporary stub.
```

---

## COMPLETED

Worker reports track completion.

```yaml
---
type: COMPLETED
track: A
beads_closed:
  - my-workflow:3-zyci.1.1
  - my-workflow:3-zyci.1.2
files_changed:
  - src/api/router.py
  - src/api/middleware.py
importance: normal
---

## Status
SUCCEEDED

## Files Changed
- `src/api/router.py` - New API router
- `src/api/middleware.py` - Validation middleware

## Key Decisions
- Used FastAPI for routing
- Implemented Pydantic validation
```

---

## FAILED

Worker reports unrecoverable failure.

```yaml
---
type: FAILED
track: A
bead_id: my-workflow:3-zyci.1.2
error: "Type error in validation schema"
recoverable: false
importance: urgent
---

## Error Details
```
TypeError: Expected Dict[str, Any], got List[str]
  at middleware.py:45
```

## Attempted Recovery
- Tried alternate schema approach
- Checked upstream types

## Files in Unknown State
- `src/api/middleware.py` - Partial changes

## Recommendation
Manual intervention required. Consider rolling back.
```

---

## STEAL

Worker requests to take over an abandoned bead.

```yaml
---
type: STEAL
bead_id: my-workflow:3-zyci.1.3
reason: "Original worker appears stalled (no heartbeat 30+ min)"
importance: high
---

## Justification
- Last heartbeat: 35 minutes ago
- Bead status: in_progress (unchanged)
- No BLOCKED message received

## Proposed Action
Take over `my-workflow:3-zyci.1.3` and continue work.

Awaiting orchestrator approval.
```

---

## RELEASE

Worker releases a bead they can no longer complete.

```yaml
---
type: RELEASE
bead_id: my-workflow:3-zyci.1.3
new_assignee: null
importance: normal
---

## Reason
Exceeded time budget for this track.

## Current State
- 40% complete
- Tests passing for completed portion
- No uncommitted changes

## Notes for Next Worker
Start from `validate_schema()` function.
```

---

## ESCALATE

Worker escalates issue to orchestrator/human.

```yaml
---
type: ESCALATE
issue: "Conflicting requirements in spec"
context: "Spec says use REST, but tech-stack.md specifies GraphQL"
suggested_action: "Clarify API style preference"
importance: urgent
---

## Conflict Details
- `spec.md` line 45: "Expose REST endpoints"
- `tech-stack.md` line 12: "Use GraphQL for all APIs"

## Impact
Cannot proceed without resolution.

## Options
1. Use REST (per spec)
2. Use GraphQL (per tech-stack)
3. Use both (hybrid approach)

Awaiting decision.
```

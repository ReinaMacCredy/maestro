# Autonomous Execution (Ralph)

Third execution mode alongside `ci` (implement) and `co` (orchestrate).

## Trigger

- `ca` or `/conductor-autonomous`

## Purpose

Invoke Ralph autonomous loop to iterate through stories with automated verification passes.

## Prerequisites

- Track exists with `metadata.json`
- `ralph.enabled == true` in metadata.json
- `ralph.stories` array is populated
- `ralph.active == false` (no other ca running)

## Preflight Checks

| Check | Condition | Action on Failure |
|-------|-----------|-------------------|
| Track exists | `metadata.json` present | HALT: "Track not found" |
| Ralph enabled | `ralph.enabled == true` | HALT: "Ralph not enabled for this track" |
| Ralph available | `ralph.active == false` | HALT: "Ralph already running" |
| Stories exist | `ralph.stories` non-empty | HALT: "No stories to execute" |

### Blocking Other Commands

When `ralph.active == true`:

| Command | Behavior |
|---------|----------|
| `ci` | HALT: "Ralph execution in progress" |
| `co` | HALT: "Ralph execution in progress" |
| `ca` | HALT: "Ralph already running" |

## Execution Flow

```
1. Set ralph.active = true (exclusive lock)
2. Invoke: toolboxes/ralph/ralph.sh <track-path> [max_iterations]
3. Ralph iterates through stories:
   - Pick next story from ralph.stories
   - Execute story implementation
   - Run verification passes
   - Update ralph.stories[id].passes status
4. On completion signal or max iterations:
   - Set ralph.active = false
   - Update workflow.state = DONE
   - Write final progress.txt
```

## Invocation

```bash
# Basic invocation
toolboxes/ralph/ralph.sh conductor/tracks/<track_id>

# With iteration limit
toolboxes/ralph/ralph.sh conductor/tracks/<track_id> 10
```

## State Updates

### On Start

```json
{
  "ralph": {
    "active": true,
    "startedAt": "2025-01-08T12:00:00Z"
  }
}
```

### On Completion

```json
{
  "ralph": {
    "active": false,
    "completedAt": "2025-01-08T14:30:00Z"
  },
  "workflow": {
    "state": "DONE"
  }
}
```

## Progress Tracking

Progress is written to `<track>/progress.txt`:

```
Story 1/5: auth-login ✓
Story 2/5: auth-logout ✓  
Story 3/5: session-mgmt ~ (in progress)
Story 4/5: password-reset -
Story 5/5: mfa-setup -
```

Story completion updates `ralph.stories[id].passes`:

```json
{
  "ralph": {
    "stories": [
      { "id": "auth-login", "passes": true },
      { "id": "auth-logout", "passes": true },
      { "id": "session-mgmt", "passes": null }
    ]
  }
}
```

## Lock Behavior

The `ralph.active` flag provides an exclusive lock:

- Prevents concurrent `ci`/`co` execution during autonomous mode
- Prevents multiple `ca` invocations
- Lock is released on completion or error

**Recovery:** If Ralph crashes, manually set `ralph.active = false` in metadata.json.

## Error Handling

| Error | Action |
|-------|--------|
| Story fails verification | Continue to next story, mark as failed |
| Max iterations reached | Exit gracefully, update state |
| Script crash | Lock remains - manual recovery needed |

## Related

- [implement.md](implement.md) - Standard implementation workflow
- [../../beads/integration.md](../beads/integration.md) - Beads integration points
- [../../../orchestrator/SKILL.md](../../../orchestrator/SKILL.md) - Parallel execution

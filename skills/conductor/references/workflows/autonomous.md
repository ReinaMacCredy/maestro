# Autonomous Execution (Ralph)

Third execution mode alongside `ci` (implement) and `co` (orchestrate).

## Trigger

- `ca` or `/conductor-autonomous`

## Purpose

Invoke Ralph autonomous loop to iterate through stories with automated verification passes.

## ⚠️ MANDATORY: Direct Shell Invocation

**When `ca` is triggered, ALWAYS run the shell script directly:**

```bash
./toolboxes/ralph/ralph.sh <track-path> [max_iterations]
```

Do NOT use Task() or sub-agents. Ralph.sh spawns fresh Amp instances for each iteration - this is the core pattern.

## Prerequisites

- Track exists with `metadata.json`
- `ralph.enabled == true` in metadata.json
- `ralph.stories` object is populated
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

When user types `ca`:

1. **Find active track** - Look for track with `ralph.enabled == true`
2. **Run preflight checks** - Validate prerequisites
3. **Execute directly via Bash:**

```bash
./toolboxes/ralph/ralph.sh conductor/tracks/<track_id> 10
```

4. Ralph.sh handles:
   - Setting `ralph.active = true` (exclusive lock)
   - Spawning fresh Amp instances per iteration
   - Updating `ralph.stories[id].passes` status
   - Releasing lock on completion/error

## Invocation Examples

```bash
# Basic invocation (default 10 iterations)
./toolboxes/ralph/ralph.sh conductor/tracks/<track_id>

# With custom iteration limit
./toolboxes/ralph/ralph.sh conductor/tracks/<track_id> 5

# Example with real track
./toolboxes/ralph/ralph.sh conductor/tracks/ralph-test_20260109 10
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
    "stories": {
      "auth-login": { "id": "auth-login", "title": "Login flow", "priority": 1, "passes": true, "beadId": "proj-abc1" },
      "auth-logout": { "id": "auth-logout", "title": "Logout flow", "priority": 2, "passes": true, "beadId": "proj-def2" },
      "session-mgmt": { "id": "session-mgmt", "title": "Session management", "priority": 3, "passes": false, "beadId": "proj-ghi3" }
    }
  }
}
```

**Note:** `ralph.stories` is an **object keyed by story ID** (not an array). This enables direct lookup: `.ralph.stories[$id].passes = true`.

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
